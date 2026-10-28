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
fn matrix_indexing_is_row_column_from_one() {
    let m = Matrix33::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0);
    assert_eq!(m[(1, 2)], 2.0);
    assert_eq!(m[(3, 1)], 7.0);
    assert_eq!(m.transposed()[(1, 2)], 4.0);
    assert_eq!(m.determinant(), -3.0);
    let id = m * m.inverse();
    for r in 1..=3 {
        for c in 1..=3 {
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
    assert!(close(should_be_identity[(2, 2)], 1.0, 1e-15));
    assert!(close(q.sin_euler(E_THT), libm::sin(-0.4), 1e-15));
    // Different pivots and normalisation: equal only to rounding.
    let (a, b) = (Quaternion::from_matrix(&q.t()), q.t().quaternion());
    for i in 1..=4 {
        assert!(close(a[i], b[i], 1e-15));
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
    locked[(1, 3)] = -1.0;
    let e = locked.euler();
    assert_eq!(e[E_THT], std::f64::consts::FRAC_PI_2);
    assert_eq!(e[E_PSI], 0.0);
}

#[test]
fn mutation_invalidates_cached_derivatives() {
    let mut q = Quaternion::from_euler(0.1, 0.2, 0.3);
    let before = q.euler();
    q.set(1, -q[1]);
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
    for i in 1..=3 {
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
