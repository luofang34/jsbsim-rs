// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Edward A Williams <Ed_Williams@compuserve.com>.
// Derived from JSBSim src/simgear/magvar/coremag.{hxx,cxx} (SimGear's
// WMM2005 implementation).
//
//! The WMM2005 magnetic field model JSBSim bundles: a twelfth-order
//! spherical harmonic expansion with linear secular variation.

/// Semi-major axis of the WGS84 ellipsoid, km.
const A: f64 = 6378.137;
/// Semi-minor axis of the WGS84 ellipsoid, km.
const B: f64 = 6356.7523142;
/// Standard Earth magnetic reference radius, km.
const R_0: f64 = 6371.2;
/// Degree and order of the expansion.
const NMAX: usize = 12;

type Coefficients = [[f64; 13]; 13];

const Z13: [f64; 13] = [0.0; 13];

#[rustfmt::skip]
const GNM_WMM2005: Coefficients = [
    Z13,
    [-29556.8, -1671.7, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [-2340.6, 3046.9, 1657.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1335.4, -2305.1, 1246.7, 674.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [919.8, 798.1, 211.3, -379.4, 100.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [-227.4, 354.6, 208.7, -136.5, -168.3, -14.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [73.2, 69.7, 76.7, -151.2, -14.9, 14.6, -86.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [80.1, -74.5, -1.4, 38.5, 12.4, 9.5, 5.7, 1.8, 0.0, 0.0, 0.0, 0.0, 0.0],
    [24.9, 7.7, -11.6, -6.9, -18.2, 10.0, 9.2, -11.6, -5.2, 0.0, 0.0, 0.0, 0.0],
    [5.6, 9.9, 3.5, -7.0, 5.1, -10.8, -1.3, 8.8, -6.7, -9.1, 0.0, 0.0, 0.0],
    [-2.3, -6.3, 1.6, -2.6, 0.0, 3.1, 0.4, 2.1, 3.9, -0.1, -2.3, 0.0, 0.0],
    [2.8, -1.6, -1.7, 1.7, -0.1, 0.1, -0.7, 0.7, 1.8, 0.0, 1.1, 4.1, 0.0],
    [-2.4, -0.4, 0.2, 0.8, -0.3, 1.1, -0.5, 0.4, -0.3, -0.3, -0.1, -0.3, -0.1],
];

#[rustfmt::skip]
const HNM_WMM2005: Coefficients = [
    Z13,
    [0.0, 5079.8, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, -2594.7, -516.7, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, -199.9, 269.3, -524.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 281.5, -226.0, 145.8, -304.7, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 42.4, 179.8, -123.0, -19.5, 103.6, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, -20.3, 54.7, 63.6, -63.4, -0.1, 50.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, -61.5, -22.4, 7.2, 25.4, 11.0, -26.4, -5.1, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 11.2, -21.0, 9.6, -19.8, 16.1, 7.7, -12.9, -0.2, 0.0, 0.0, 0.0, 0.0],
    [0.0, -20.1, 12.9, 12.6, -6.7, -8.1, 8.0, 2.9, -7.9, 6.0, 0.0, 0.0, 0.0],
    [0.0, 2.4, 0.2, 4.4, 4.8, -6.5, -1.1, -3.4, -0.8, -2.3, -7.9, 0.0, 0.0],
    [0.0, 0.3, 1.2, -0.8, -2.5, 0.9, -0.6, -2.7, -0.9, -1.3, -2.0, -1.2, 0.0],
    [0.0, -0.4, 0.3, 2.4, -2.6, 0.6, 0.3, 0.0, 0.0, 0.3, -0.9, -0.4, 0.8],
];

#[rustfmt::skip]
const GTNM_WMM2005: Coefficients = [
    Z13,
    [8.0, 10.6, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [-15.1, -7.8, -0.8, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.4, -2.6, -1.2, -6.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [-2.5, 2.8, -7.0, 6.2, -3.8, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [-2.8, 0.7, -3.2, -1.1, 0.1, -0.8, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [-0.7, 0.4, -0.3, 2.3, -2.1, -0.6, 1.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.2, -0.1, -0.3, 1.1, 0.6, 0.5, -0.4, 0.6, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.1, 0.3, -0.4, 0.3, -0.3, 0.2, 0.4, -0.7, 0.4, 0.0, 0.0, 0.0, 0.0],
    Z13,
    Z13,
    Z13,
    Z13,
];

#[rustfmt::skip]
const HTNM_WMM2005: Coefficients = [
    Z13,
    [0.0, -20.9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, -23.2, -14.6, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 5.0, -7.0, -0.6, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 2.2, 1.6, 5.8, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 1.7, 2.1, 4.8, -1.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, -0.6, -1.9, -0.4, -0.5, -0.3, 0.7, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.6, 0.4, 0.2, 0.3, -0.8, -0.2, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, -0.2, 0.1, 0.3, 0.4, 0.1, -0.2, 0.4, 0.4, 0.0, 0.0, 0.0, 0.0],
    Z13,
    Z13,
    Z13,
    Z13,
];

/// A 13 by 13 grid indexed by degree `n` and order `m`; out-of-range
/// entries read 0 and ignore writes.
#[derive(Clone, Copy)]
struct Grid(Coefficients);

impl Grid {
    fn at(&self, n: usize, m: usize) -> f64 {
        self.0
            .get(n)
            .and_then(|row| row.get(m))
            .copied()
            .unwrap_or(0.0)
    }

    fn set(&mut self, n: usize, m: usize, v: f64) {
        if let Some(x) = self.0.get_mut(n).and_then(|row| row.get_mut(m)) {
            *x = v;
        }
    }
}

/// `yymmdd_to_julian_days`: the Julian day of a date of 1950 to 2049,
/// with `yy` the year since 1900 (or since 2000 below 50).
#[must_use]
pub fn yymmdd_to_julian_days(yy: i64, mm: i64, dd: i64) -> i64 {
    let yy = if yy < 50 { 2000 + yy } else { 1900 + yy };
    let mut jd = dd - 32075 + 1461 * (yy + 4800 + (mm - 14) / 12) / 4;
    jd += 367 * (mm - 2 - (mm - 14) / 12 * 12) / 12;
    jd -= 3 * ((yy + 4900 + (mm - 14) / 12) / 100) / 4;
    jd
}

/// The associated Legendre functions `P` of `cos(theta)` and their
/// derivatives `DP`, from `c = cos(theta)` and `s = sin(theta)`.
fn legendre(c: f64, s: f64) -> (Grid, Grid) {
    let (mut p, mut dp) = (Grid([[0.0; 13]; 13]), Grid([[0.0; 13]; 13]));
    p.set(0, 0, 1.0);
    p.set(1, 1, s);
    dp.set(1, 1, c);
    p.set(1, 0, c);
    dp.set(1, 0, -s);
    for n in 2..=NMAX {
        let nf = n as f64;
        let root = libm::sqrt((2.0 * nf - 1.0) / (2.0 * nf));
        let (p1, dp1) = (p.at(n - 1, n - 1), dp.at(n - 1, n - 1));
        p.set(n, n, p1 * s * root);
        dp.set(n, n, (dp1 * s + p1 * c) * root);
    }
    for m in 0..=NMAX {
        let mm = (m * m) as f64;
        for n in (m + 1).max(2)..=NMAX {
            let nf = n as f64;
            let root0 = libm::sqrt(((n - 1) * (n - 1)) as f64 - mm);
            let root1 = 1.0 / libm::sqrt((n * n) as f64 - mm);
            let (p1, p2, dp1, dp2) = (
                p.at(n - 1, m),
                p.at(n - 2, m),
                dp.at(n - 1, m),
                dp.at(n - 2, m),
            );
            p.set(n, m, (p1 * c * (2.0 * nf - 1.0) - p2 * root0) * root1);
            dp.set(
                n,
                m,
                ((dp1 * c - p1 * s) * (2.0 * nf - 1.0) - dp2 * root0) * root1,
            );
        }
    }
    (p, dp)
}

/// The Gauss coefficients `gnm` and `hnm` `yearfrac` years after the
/// model epoch.
fn coefficients(yearfrac: f64) -> (Grid, Grid) {
    let (mut g, mut h) = (Grid([[0.0; 13]; 13]), Grid([[0.0; 13]; 13]));
    for n in 1..=NMAX {
        for m in 0..=NMAX {
            let (g0, gt) = (Grid(GNM_WMM2005).at(n, m), Grid(GTNM_WMM2005).at(n, m));
            let (h0, ht) = (Grid(HNM_WMM2005).at(n, m), Grid(HTNM_WMM2005).at(n, m));
            g.set(n, m, g0 + yearfrac * gt);
            h.set(n, m, h0 + yearfrac * ht);
        }
    }
    (g, h)
}

/// `calc_magvar`'s field at geodetic latitude `lat` and longitude `lon`
/// (rad), height `h` (km) and Julian day `dat`: `B_r`, `B_theta`, `B_phi`
/// and the north, east and down components, nT.
#[must_use]
pub fn calc_magvar(lat: f64, lon: f64, h: f64, dat: i64) -> [f64; 6] {
    let date0_wmm2005 = yymmdd_to_julian_days(5, 1, 1);
    let sinlat = libm::sin(lat);
    let coslat = libm::cos(lat);
    let sr = libm::sqrt(A * A * coslat * coslat + B * B * sinlat * sinlat);
    let theta = libm::atan2(coslat * (h * sr + A * A), sinlat * (h * sr + B * B));
    let a4 = A * A * A * A;
    let r = h * h
        + 2.0 * h * sr
        + (a4 - (a4 - B * B * B * B) * sinlat * sinlat)
            / (A * A - (A * A - B * B) * sinlat * sinlat);
    let r = libm::sqrt(r);
    let c = libm::cos(theta);
    let s = libm::sin(theta);
    let inv_s = 1.0 / (s + if s == 0.0 { 1.0e-8 } else { 0.0 });
    let (p, dp) = legendre(c, s);
    let yearfrac = (dat - date0_wmm2005) as f64 / 365.25;
    let (gnm, hnm) = coefficients(yearfrac);
    let mut sm = [0.0; 13];
    let mut cm = [0.0; 13];
    for (m, (sm, cm)) in sm.iter_mut().zip(cm.iter_mut()).enumerate() {
        *sm = libm::sin(m as f64 * lon);
        *cm = libm::cos(m as f64 * lon);
    }
    let (mut b_r, mut b_theta, mut b_phi) = (0.0, 0.0, 0.0);
    let fn_0 = R_0 / r;
    let mut fnn = fn_0 * fn_0;
    for n in 1..=NMAX {
        let (mut c1_n, mut c2_n, mut c3_n) = (0.0, 0.0, 0.0);
        for (m, (&sm, &cm)) in sm.iter().zip(cm.iter()).enumerate().take(n + 1) {
            let (g, h) = (gnm.at(n, m), hnm.at(n, m));
            let tmp = g * cm + h * sm;
            c1_n += tmp * p.at(n, m);
            c2_n += tmp * dp.at(n, m);
            c3_n += m as f64 * (g * sm - h * cm) * p.at(n, m);
        }
        fnn *= fn_0;
        b_r += (n + 1) as f64 * c1_n * fnn;
        b_theta -= c2_n * fnn;
        b_phi += c3_n * fnn * inv_s;
    }
    let psi = theta - (std::f64::consts::PI / 2.0 - lat);
    let (sinpsi, cospsi) = (libm::sin(psi), libm::cos(psi));
    let x = -b_theta * cospsi - b_r * sinpsi;
    let y = b_phi;
    let z = b_theta * sinpsi - b_r * cospsi;
    [b_r, b_theta, b_phi, x, y, z]
}
