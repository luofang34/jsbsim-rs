// SPDX-License-Identifier: LGPL-2.1-or-later
// Derived from GeographicLib src/Geodesic.cpp, as bundled with JSBSim in
// src/GeographicLib: Copyright (c) Charles Karney (2009-2021)
// <charles@karney.com>, licensed under the MIT/X11 License, whose notice
// (GeographicLib's LICENSE.txt) follows.
//
//   The MIT License (MIT).
//
//   Copyright (c) 2008-2022, Charles Karney
//
//   Permission is hereby granted, free of charge, to any person
//   obtaining a copy of this software and associated documentation
//   files (the "Software"), to deal in the Software without
//   restriction, including without limitation the rights to use, copy,
//   modify, merge, publish, distribute, sublicense, and/or sell copies
//   of the Software, and to permit persons to whom the Software is
//   furnished to do so, subject to the following conditions:
//
//   The above copyright notice and this permission notice shall be
//   included in all copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//   EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//   MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
//   NONINFRINGEMENT.  IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT
//   HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
//   WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//   OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//   DEALINGS IN THE SOFTWARE.
//
//! `GenInverse`: the inverse problem brought to canonical form, then
//! solved along a meridian, along the equator, directly for short lines,
//! or by Newton's method on alpha1.

use super::angles::{DEGREE, ang_diff, ang_round, cmax, lat_fix, norm, sincosd, sq};
use super::series::{Ends, LengthMask, lengths};
use super::solve::{Bracket, Start};
use super::{Geodesic, MAXIT1, MAXIT2};

/// `GenInverse` outputs before the azimuths become degrees.
#[derive(Debug, Clone, Copy)]
pub(super) struct Inverse {
    /// Distance; NaN unless requested.
    pub(super) s12: f64,
    pub(super) salp1: f64,
    pub(super) calp1: f64,
    pub(super) salp2: f64,
    pub(super) calp2: f64,
}

/// The points brought to canonical form: 0 <= lon12 <= 180,
/// -90 <= lat1 <= 0 and lat1 <= lat2 <= -lat1, with `beta` the reduced
/// latitudes and `dn` the `sqrt(1 + ep2 sin(beta)^2)` factors.
#[derive(Debug, Clone, Copy)]
pub(super) struct Canonical {
    pub(super) lon12s: f64,
    pub(super) lam12: f64,
    pub(super) slam12: f64,
    pub(super) clam12: f64,
    pub(super) lat1: f64,
    pub(super) sbet1: f64,
    pub(super) cbet1: f64,
    pub(super) sbet2: f64,
    pub(super) cbet2: f64,
    pub(super) dn1: f64,
    pub(super) dn2: f64,
    swapp: f64,
    latsign: f64,
    lonsign: f64,
}

impl Geodesic {
    /// `GenInverse` with the output mask `DISTANCE` (`distance`) or
    /// `AZIMUTH`.
    pub(super) fn gen_inverse(
        &self,
        lat1: f64,
        lon1: f64,
        lat2: f64,
        lon2: f64,
        distance: bool,
    ) -> Inverse {
        let k = self.canonical(lat1, lon1, lat2, lon2);
        let mut r = Inverse {
            s12: f64::NAN,
            salp1: 0.0,
            calp1: 0.0,
            salp2: 0.0,
            calp2: 0.0,
        };
        let mut meridian = k.lat1 == -90.0 || k.slam12 == 0.0;
        let mut s12x = f64::NAN;
        if meridian {
            match self.meridian(&k) {
                Some((s12, alp)) => {
                    s12x = s12;
                    (r.salp1, r.calp1, r.salp2, r.calp2) = alp;
                }
                None => meridian = false,
            }
        }
        if !meridian && k.sbet1 == 0.0 && (self.f <= 0.0 || k.lon12s >= self.f * 180.0) {
            // Geodesic runs along the equator.
            (r.calp1, r.calp2, r.salp1, r.salp2) = (0.0, 0.0, 1.0, 1.0);
            s12x = self.a * k.lam12;
        } else if !meridian {
            let start = self.inverse_start(&k);
            (r.salp1, r.calp1) = (start.salp1, start.calp1);
            if start.sig12 >= 0.0 {
                // Short lines: InverseStart set alpha2 and dnm.
                (r.salp2, r.calp2) = (start.salp2, start.calp2);
                s12x = start.sig12 * self.b * start.dnm;
            } else {
                let newton = self.newton(&k, &start, distance);
                s12x = newton.0;
                (r.salp1, r.calp1, r.salp2, r.calp2) = newton.1;
            }
        }
        if distance {
            r.s12 = 0.0 + s12x;
        }
        if k.swapp < 0.0 {
            core::mem::swap(&mut r.salp1, &mut r.salp2);
            core::mem::swap(&mut r.calp1, &mut r.calp2);
        }
        r.salp1 *= k.swapp * k.lonsign;
        r.calp1 *= k.swapp * k.latsign;
        r.salp2 *= k.swapp * k.lonsign;
        r.calp2 *= k.swapp * k.latsign;
        r
    }

    /// The canonical form of the points (the first part of `GenInverse`).
    fn canonical(&self, lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> Canonical {
        // Longitude difference, AngDiff exact, made positive; close to the
        // same half-meridian is made exactly so.
        let (lon12, lon12s) = ang_diff(lon1, lon2);
        let mut lonsign: i32 = if lon12 >= 0.0 { 1 } else { -1 };
        let lon12 = f64::from(lonsign) * ang_round(lon12);
        let lon12s = ang_round((180.0 - lon12) - f64::from(lonsign) * lon12s);
        let lam12 = lon12 * DEGREE;
        let (slam12, clam12) = if lon12 > 90.0 {
            let (s, c) = sincosd(lon12s);
            (s, -c)
        } else {
            sincosd(lon12)
        };
        // Really close to the equator is treated as on it.
        let mut lat1 = ang_round(lat_fix(lat1));
        let mut lat2 = ang_round(lat_fix(lat2));
        // The point of higher latitude, or a NaN latitude, becomes point 1.
        let swapp: i32 = if lat1.abs() < lat2.abs() { -1 } else { 1 };
        if swapp < 0 {
            lonsign *= -1;
            core::mem::swap(&mut lat1, &mut lat2);
        }
        let latsign: i32 = if lat1 < 0.0 { 1 } else { -1 };
        lat1 *= f64::from(latsign);
        lat2 *= f64::from(latsign);

        let (s, c) = sincosd(lat1);
        let (sbet1, cbet1) = norm(s * self.f1, c);
        // cbet1 = +epsilon at the poles.
        let cbet1 = cmax(self.tiny, cbet1);
        let (s, c) = sincosd(lat2);
        let (mut sbet2, cbet2) = norm(s * self.f1, c);
        let mut cbet2 = cmax(self.tiny, cbet2);
        // Force bet2 = +/- bet1 exactly where the sensitive measure of
        // |bet1| - |bet2| vanishes (see the C++ for the failing case).
        if cbet1 < -sbet1 {
            if cbet2 == cbet1 {
                sbet2 = if sbet2 < 0.0 { sbet1 } else { -sbet1 };
            }
        } else if sbet2.abs() == -sbet1 {
            cbet2 = cbet1;
        }
        Canonical {
            lon12s,
            lam12,
            slam12,
            clam12,
            lat1,
            sbet1,
            cbet1,
            sbet2,
            cbet2,
            dn1: libm::sqrt(1.0 + self.ep2 * sq(sbet1)),
            dn2: libm::sqrt(1.0 + self.ep2 * sq(sbet2)),
            swapp: f64::from(swapp),
            latsign: f64::from(latsign),
            lonsign: f64::from(lonsign),
        }
    }

    /// Endpoints on one full meridian: the distance and (salp1, calp1,
    /// salp2, calp2), or `None` when the meridian is not the shortest path
    /// (prolate and nearly antipodal).
    fn meridian(&self, k: &Canonical) -> Option<(f64, (f64, f64, f64, f64))> {
        // Head to the target longitude; at the target, head north.
        let (calp1, salp1) = (k.clam12, k.slam12);
        let (calp2, salp2) = (1.0, 0.0);
        let e = Ends {
            ssig1: k.sbet1,
            csig1: calp1 * k.cbet1,
            dn1: k.dn1,
            ssig2: k.sbet2,
            csig2: calp2 * k.cbet2,
            dn2: k.dn2,
        };
        let sig12 = libm::atan2(
            cmax(0.0, e.csig1 * e.ssig2 - e.ssig1 * e.csig2),
            e.csig1 * e.csig2 + e.ssig1 * e.ssig2,
        );
        let mask = LengthMask {
            distance: true,
            reduced_length: true,
        };
        let l = lengths(self.n, sig12, &e, mask);
        let (mut s12x, m12x) = (l.s12b, l.m12b);
        // A meridian longer than half way round with m12 < 0 is not the
        // shortest path; zero-length geodesics may give m12 < 0 too.
        if !(sig12 < 1.0 || m12x >= 0.0) {
            return None;
        }
        // GenInverse also zeroes sig12 and m12 here, which feed only the
        // arc length and reduced length outputs.
        if sig12 < 3.0 * self.tiny || (sig12 < self.tol0 && (s12x < 0.0 || m12x < 0.0)) {
            s12x = 0.0;
        }
        s12x *= self.b;
        Some((s12x, (salp1, calp1, salp2, calp2)))
    }

    /// Newton's method on alpha1 from the starting guess, bracketed, with
    /// bisection when a step leaves the bracket: the distance and (salp1,
    /// calp1, salp2, calp2).
    fn newton(&self, k: &Canonical, start: &Start, distance: bool) -> (f64, (f64, f64, f64, f64)) {
        let (mut salp1, mut calp1) = (start.salp1, start.calp1);
        let mut bracket = Bracket::new(self.tiny);
        let mut last = None;
        let (mut tripn, mut tripb) = (false, false);
        for numit in 0..MAXIT2 {
            let l = self.lambda12(k, salp1, calp1, numit < MAXIT1);
            let v = l.lam12;
            last = Some(l);
            // Converged, or escaping with a NaN.
            let threshold = (if tripn { 8.0 } else { 1.0 }) * self.tol0;
            if tripb || v.abs() < threshold || v.is_nan() {
                break;
            }
            bracket.update(v, salp1, calp1, numit > MAXIT1);
            if numit < MAXIT1 && l.dlam12 > 0.0 {
                let dalp1 = -v / l.dlam12;
                let (sdalp1, cdalp1) = (libm::sin(dalp1), libm::cos(dalp1));
                let nsalp1 = salp1 * cdalp1 + calp1 * sdalp1;
                if nsalp1 > 0.0 && dalp1.abs() < super::angles::PI {
                    calp1 = calp1 * cdalp1 - salp1 * sdalp1;
                    (salp1, calp1) = norm(nsalp1, calp1);
                    // Convergence on epsilon where the slope tends to 0.
                    tripn = v.abs() <= 16.0 * self.tol0;
                    continue;
                }
            }
            // Bisect where dv is not positive or the step leaves (0, pi).
            (salp1, calp1) = bracket.midpoint();
            tripn = false;
            tripb = bracket.collapsed(salp1, calp1, self.tolb);
        }
        let Some(l) = last else {
            return (f64::NAN, (salp1, calp1, start.salp2, start.calp2));
        };
        let mask = LengthMask {
            distance,
            reduced_length: false,
        };
        let s12x = lengths(l.eps, l.sig12, &l.ends, mask).s12b * self.b;
        (s12x, (salp1, calp1, l.salp2, l.calp2))
    }
}
