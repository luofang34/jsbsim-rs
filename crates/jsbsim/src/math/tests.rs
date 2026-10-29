// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

const WGS84_A: f64 = 20_925_646.325_46;
const WGS84_B: f64 = 20_855_486.595_2;

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn vector_basics() {
    let u = ColumnVector3::new(1.0, 2.0, 2.0);
    assert_eq!(u.magnitude(), 3.0);
    assert_eq!(
        u.cross(ColumnVector3::new(0.0, 0.0, 1.0)),
        ColumnVector3::new(2.0, -1.0, 0.0)
    );
    assert_eq!(dot_product(u, u), 9.0);
    assert_eq!(u / 0.0, ColumnVector3::zero());
    let mut z = ColumnVector3::zero();
    z.normalize();
    assert_eq!(z, ColumnVector3::zero());
    assert_eq!(u[E_Z], 2.0);
}

#[test]
fn matrix_indexing_is_row_column() {
    let m = Matrix33::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0);
    assert_eq!(m[(E_X, E_Y)], 2.0);
    assert_eq!(m[(E_Z, E_X)], 7.0);
    assert_eq!(m.transposed()[(E_X, E_Y)], 4.0);
    assert_eq!(m.determinant(), -3.0);
    let id = m * m.inverse();
    for r in Axis::ALL {
        for c in Axis::ALL {
            assert!(close(id[(r, c)], f64::from(u8::from(r == c)), 1e-12));
        }
    }
    assert_eq!(Matrix33::zero().inverse(), Matrix33::zero());
}

#[test]
fn euler_round_trip_and_orthonormal_transform() {
    let q = Quaternion::from_euler(0.3, -0.4, 2.5);
    let e = q.euler();
    assert!(close(e[E_PHI], 0.3, 1e-14));
    assert!(close(e[E_THT], -0.4, 1e-14));
    assert!(close(e[E_PSI], 2.5, 1e-14));
    let should_be_identity = q.t() * q.t_inv();
    assert!(close(should_be_identity[(E_Y, E_Y)], 1.0, 1e-15));
    assert!(close(q.sin_euler(E_THT), libm::sin(-0.4), 1e-15));
    // Different pivots and normalisation: equal only to rounding.
    let (a, b) = (Quaternion::from_matrix(&q.t()), q.t().quaternion());
    for (a, b) in a.to_array().into_iter().zip(b.to_array()) {
        assert!(close(a, b, 1e-15));
    }
}

#[test]
fn negative_heading_is_wrapped_into_zero_to_two_pi() {
    let q = Quaternion::from_euler(0.0, 0.0, -0.5);
    assert!(close(
        q.euler()[E_PSI],
        2.0 * std::f64::consts::PI - 0.5,
        1e-14
    ));
}

#[test]
fn gimbal_lock_sets_heading_to_zero() {
    let q = Quaternion::from_euler(0.2, std::f64::consts::FRAC_PI_2, 0.7);
    let m = q.t();
    let mut locked = m;
    locked[(E_X, E_Z)] = -1.0;
    let e = locked.euler();
    assert_eq!(e[E_THT], std::f64::consts::FRAC_PI_2);
    assert_eq!(e[E_PSI], 0.0);
}

#[test]
fn mutation_invalidates_cached_derivatives() {
    let mut q = Quaternion::from_euler(0.1, 0.2, 0.3);
    let before = q.euler();
    q *= 2.0;
    assert_ne!(q.euler(), before);
}

#[test]
fn geodetic_round_trip_on_wgs84() {
    let mut l = Location::default();
    l.set_ellipse(WGS84_A, WGS84_B);
    l.set_position_geodetic(-1.2, 0.6, 12_000.0);
    assert!(close(l.geod_latitude_rad(), 0.6, 1e-14));
    assert!(close(l.geod_altitude(), 12_000.0, 1e-6));
    assert!(close(l.longitude(), -1.2, 1e-15));
}

#[test]
fn local_frame_round_trip() {
    let l = Location::from_spherical(0.4, -0.3, 2.0e7);
    let v = ColumnVector3::new(10.0, -20.0, 30.0);
    let moved = l.local_to_location(v);
    let back = l.location_to_local(moved.ecef());
    for i in Axis::ALL {
        assert!(close(back[i], v[i], 1e-7));
    }
}

#[test]
fn degenerate_locations() {
    let mut origin = Location::from_ecef(ColumnVector3::zero());
    assert_eq!(origin.longitude(), 0.0);
    assert_eq!(origin.latitude(), 0.0);
    origin.set_ellipse(WGS84_A, WGS84_B);
    assert_eq!(origin.geod_altitude(), -WGS84_A);
    assert_eq!(Location::default().sea_level_radius(), 1.0);
}

#[test]
fn axes_outside_one_to_three_are_unrepresentable() {
    for i in [0, 4, usize::MAX] {
        assert_eq!(Axis::from_index1(i), None);
    }
    for (i, axis) in Axis::ALL.into_iter().enumerate() {
        assert_eq!(axis.index1(), i + 1);
        assert_eq!(Axis::from_index1(i + 1), Some(axis));
    }
    let v = ColumnVector3::new(1.0, 2.0, 3.0);
    assert_eq!(Axis::ALL.map(|a| v[a]), v.to_array());
    let m = Matrix33::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
    for r in Axis::ALL {
        for c in Axis::ALL {
            let entry = 3 * (r.index1() - 1) + c.index1();
            assert_eq!(m[(r, c)], entry as f64);
        }
    }
}

#[test]
fn cached_and_uncached_reads_are_bit_identical() {
    const AXES: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];
    let flat = |m: Matrix33| AXES.into_iter().flat_map(move |r| AXES.map(|c| m[(r, c)]));
    let mut l = Location::from_ecef(ColumnVector3::new(1.2e7, -3.4e6, 1.5e7));
    l.set_ellipse(WGS84_A, WGS84_B);
    let read = |l: &Location| {
        let mut v = vec![
            l.longitude(),
            l.latitude(),
            l.radius(),
            l.geod_latitude_rad(),
            l.geod_altitude(),
            l.sea_level_radius(),
            l.sin_longitude(),
            l.cos_longitude(),
        ];
        v.extend(flat(l.tec2l()));
        v.extend(flat(l.tl2ec()));
        v.into_iter().map(f64::to_bits).collect::<Vec<_>>()
    };
    let uncached = read(&l);
    l.cache_derived();
    assert_eq!(read(&l), uncached);

    let mut q = Quaternion::from_euler(0.3, -1.1, 2.9);
    let read = |q: &Quaternion| {
        let mut v = Vec::new();
        v.extend(flat(q.t()));
        v.extend(flat(q.t_inv()));
        v.extend(q.euler().to_array());
        for i in AXES {
            v.extend([q.sin_euler(i), q.cos_euler(i)]);
        }
        v.into_iter().map(f64::to_bits).collect::<Vec<_>>()
    };
    let uncached = read(&q);
    q.cache_derived();
    assert_eq!(read(&q), uncached);
    q *= 0.5;
    assert_eq!(read(&q), read(&(&q * 1.0)), "a mutation drops the cache");
}

/// The port calls `libm::sincos` wherever it takes the sine and the cosine
/// of one angle, relying on it to return exactly `libm::sin` and
/// `libm::cos`, which the other sites call.
#[test]
fn libm_sincos_is_sin_and_cos_bit_for_bit() {
    let check = |x: f64| {
        let (s, c) = libm::sincos(x);
        let bits = x.to_bits();
        assert_eq!(
            s.to_bits(),
            libm::sin(x).to_bits(),
            "sin({x:e}) [{bits:#x}]"
        );
        assert_eq!(
            c.to_bits(),
            libm::cos(x).to_bits(),
            "cos({x:e}) [{bits:#x}]"
        );
    };
    let neighbours = |x: f64, ulps: u64| {
        let bits = x.to_bits();
        for d in 0..=ulps {
            check(f64::from_bits(bits.wrapping_add(d)));
            check(f64::from_bits(bits.wrapping_sub(d)));
        }
    };
    for x in [
        0.0,
        f64::INFINITY,
        f64::NAN,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::EPSILON,
    ] {
        check(x);
        check(-x);
    }
    check(f64::from_bits(0x7ff0_0000_0000_0001));
    check(f64::from_bits(0xfff8_0000_dead_beef));
    for bits in [1, 2, 0xff, 0x8_0000_0000_0000, 0xf_ffff_ffff_ffff] {
        check(f64::from_bits(bits));
        check(-f64::from_bits(bits));
    }
    // The small-argument shortcuts of sin (|x| < 2^-26) and of cos and
    // sincos (|x| < 2^-27 sqrt 2), and the edge of the reduction-free
    // range, by high word.
    for high in [0x3e46_a09e_u64, 0x3e50_0000, 0x3fe9_21fb, 0x3fe9_21fc] {
        let x = f64::from_bits(high << 32);
        neighbours(x, 4096);
        neighbours(-x, 4096);
    }
    for e in -40..=-20 {
        neighbours(libm::ldexp(1.0, e), 256);
    }
    for k in -4000..=4000_i32 {
        neighbours(f64::from(k) * core::f64::consts::FRAC_PI_4, 64);
        neighbours(f64::from(k) * core::f64::consts::FRAC_PI_2, 16);
    }
    for e in 0..=300 {
        let x = libm::pow(10.0, f64::from(e));
        neighbours(x, 8);
        neighbours(-x, 8);
    }
    // Random bit patterns, which cover every exponent, then angles of the
    // range a simulation sees.
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };
    for _ in 0..200_000 {
        check(f64::from_bits(next()));
        let unit = (next() >> 11) as f64 / (1_u64 << 53) as f64;
        check((unit - 0.5) * 20.0);
        check((unit - 0.5) * 2.0e6);
    }
}
