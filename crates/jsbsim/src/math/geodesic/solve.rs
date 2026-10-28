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
//! `InverseStart`, `Astroid` and `Lambda12`: the starting guess for
//! Newton's method on alpha1 and the function it solves.

use super::Geodesic;
use super::angles::{PI, cmax, cmin, norm, sq};
use super::inverse::Canonical;
use super::series::{Ends, REDUCED_LENGTH, lengths, sin_cos_series};

/// `InverseStart` results.
#[derive(Debug, Clone, Copy)]
pub(super) struct Start {
    /// The arc length of a really short line, else -1 (Newton's method
    /// needed).
    pub(super) sig12: f64,
    pub(super) salp1: f64,
    pub(super) calp1: f64,
    /// Set only for a really short line.
    pub(super) salp2: f64,
    pub(super) calp2: f64,
    /// Set only for a short line.
    pub(super) dnm: f64,
}

/// `Lambda12` results.
#[derive(Debug, Clone, Copy)]
pub(super) struct Lambda {
    /// lambda12(alpha1) - lambda12 of the target.
    pub(super) lam12: f64,
    pub(super) salp2: f64,
    pub(super) calp2: f64,
    pub(super) sig12: f64,
    pub(super) ends: Ends,
    pub(super) eps: f64,
    /// d lam12 / d alpha1, when asked for.
    pub(super) dlam12: f64,
}

/// The range (alpha1a, alpha1b) that brackets the root of Newton's method.
#[derive(Debug, Clone, Copy)]
pub(super) struct Bracket {
    salp1a: f64,
    calp1a: f64,
    salp1b: f64,
    calp1b: f64,
}

impl Bracket {
    pub(super) fn new(tiny: f64) -> Self {
        Self {
            salp1a: tiny,
            calp1a: 1.0,
            salp1b: tiny,
            calp1b: -1.0,
        }
    }

    /// Shrinks the bracket with the value `v` at alpha1 = (salp1, calp1).
    pub(super) fn update(&mut self, v: f64, salp1: f64, calp1: f64, past_maxit1: bool) {
        if v > 0.0 && (past_maxit1 || calp1 / salp1 > self.calp1b / self.salp1b) {
            (self.salp1b, self.calp1b) = (salp1, calp1);
        } else if v < 0.0 && (past_maxit1 || calp1 / salp1 < self.calp1a / self.salp1a) {
            (self.salp1a, self.calp1a) = (salp1, calp1);
        }
    }

    pub(super) fn midpoint(&self) -> (f64, f64) {
        norm(
            (self.salp1a + self.salp1b) / 2.0,
            (self.calp1a + self.calp1b) / 2.0,
        )
    }

    /// Whether alpha1 = (salp1, calp1) is within `tolb` of either end.
    pub(super) fn collapsed(&self, salp1: f64, calp1: f64, tolb: f64) -> bool {
        (self.salp1a - salp1).abs() + (self.calp1a - calp1) < tolb
            || (salp1 - self.salp1b).abs() + (calp1 - self.calp1b) < tolb
    }
}

/// `Astroid`: the positive root k of
/// k^4 + 2 k^3 - (x^2 + y^2 - 1) k^2 - 2 y^2 k - y^2 = 0.
fn astroid(x: f64, y: f64) -> f64 {
    let p = sq(x);
    let q = sq(y);
    let r = (p + q - 1.0) / 6.0;
    if q == 0.0 && r <= 0.0 {
        // y = 0 with |x| <= 1.
        return 0.0;
    }
    // S and the discriminant are multiplied by r^3 to avoid dividing by r.
    let s = p * q / 4.0;
    let r2 = sq(r);
    let r3 = r * r2;
    let disc = s * (s + 2.0 * r3);
    let mut u = r;
    if disc >= 0.0 {
        let mut t3 = s + r3;
        // The sign of the sqrt that maximises |T3| avoids cancellation.
        t3 += if t3 < 0.0 {
            -libm::sqrt(disc)
        } else {
            libm::sqrt(disc)
        };
        let t = libm::cbrt(t3);
        u += t + if t != 0.0 { r2 / t } else { 0.0 };
    } else {
        // T is complex, but u is real; this cube root avoids cancellation.
        let ang = libm::atan2(libm::sqrt(-disc), -(s + r3));
        u += 2.0 * r * libm::cos(ang / 3.0);
    }
    let v = libm::sqrt(sq(u) + q);
    let uv = if u < 0.0 { q / (v - u) } else { u + v };
    let w = (uv - q) / (2.0 * v);
    uv / (libm::sqrt(uv + sq(w)) + w)
}

impl Geodesic {
    /// `InverseStart`: a starting alpha1 for Newton's method, or the
    /// solution of a really short line.
    pub(super) fn inverse_start(&self, k: &Canonical) -> Start {
        let (sbet1, cbet1, sbet2, cbet2) = (k.sbet1, k.cbet1, k.sbet2, k.cbet2);
        let mut out = Start {
            sig12: -1.0,
            salp1: 0.0,
            calp1: 0.0,
            salp2: f64::NAN,
            calp2: f64::NAN,
            dnm: 1.0,
        };
        // bet12 = bet2 - bet1 in [0, pi); bet12a = bet2 + bet1 in (-pi, 0].
        let sbet12 = sbet2 * cbet1 - cbet2 * sbet1;
        let cbet12 = cbet2 * cbet1 + sbet2 * sbet1;
        let sbet12a = sbet2 * cbet1 + cbet2 * sbet1;
        let shortline = cbet12 >= 0.0 && sbet12 < 0.5 && cbet2 * k.lam12 < 0.5;
        let (somg12, comg12) = if shortline {
            let mut sbetm2 = sq(sbet1 + sbet2);
            sbetm2 /= sbetm2 + sq(cbet1 + cbet2);
            out.dnm = libm::sqrt(1.0 + self.ep2 * sbetm2);
            let omg12 = k.lam12 / (self.f1 * out.dnm);
            (libm::sin(omg12), libm::cos(omg12))
        } else {
            (k.slam12, k.clam12)
        };
        let mut salp1 = cbet2 * somg12;
        let mut calp1 = if comg12 >= 0.0 {
            sbet12 + cbet2 * sbet1 * sq(somg12) / (1.0 + comg12)
        } else {
            sbet12a - cbet2 * sbet1 * sq(somg12) / (1.0 - comg12)
        };
        let ssig12 = libm::hypot(salp1, calp1);
        let csig12 = sbet1 * sbet2 + cbet1 * cbet2 * comg12;
        if shortline && ssig12 < self.etol2 {
            // Really short lines.
            let salp2 = cbet1 * somg12;
            let calp2 = sbet12
                - cbet1
                    * sbet2
                    * if comg12 >= 0.0 {
                        sq(somg12) / (1.0 + comg12)
                    } else {
                        1.0 - comg12
                    };
            (out.salp2, out.calp2) = norm(salp2, calp2);
            out.sig12 = libm::atan2(ssig12, csig12);
        } else if self.n.abs() > 0.1
            || csig12 >= 0.0
            || ssig12 >= 6.0 * self.n.abs() * PI * sq(cbet1)
        {
            // The zeroth-order spherical approximation will do (and the
            // astroid is skipped when too eccentric).
        } else {
            (salp1, calp1) = self.antipodal_start(k, sbet12a);
        }
        // Sanity check on the guess, which lets a NaN through.
        (out.salp1, out.calp1) = if salp1 > 0.0 || salp1.is_nan() {
            norm(salp1, calp1)
        } else {
            (1.0, 0.0)
        };
        out
    }

    /// The nearly antipodal part of `InverseStart`: x and y scaled so that
    /// the antipodal point is at the origin and the singular point at
    /// y = 0, x = -1, then alpha1 from the strip near the cut or from the
    /// astroid estimate of omg12.
    fn antipodal_start(&self, k: &Canonical, sbet12a: f64) -> (f64, f64) {
        let (sbet1, cbet1, sbet2, cbet2) = (k.sbet1, k.cbet1, k.sbet2, k.cbet2);
        let lam12x = libm::atan2(-k.slam12, -k.clam12); // lam12 - pi
        let (x, y, lamscale);
        if self.f >= 0.0 {
            // x = dlong, y = dlat.
            let k2 = sq(sbet1) * self.ep2;
            let eps = k2 / (2.0 * (1.0 + libm::sqrt(1.0 + k2)) + k2);
            lamscale = self.f * cbet1 * self.a3f(eps) * PI;
            let betscale = lamscale * cbet1;
            x = lam12x / lamscale;
            y = sbet12a / betscale;
        } else {
            // x = dlat, y = dlong.
            let cbet12a = cbet2 * cbet1 - sbet2 * sbet1;
            let bet12a = libm::atan2(sbet12a, cbet12a);
            let e = Ends {
                ssig1: sbet1,
                csig1: -cbet1,
                dn1: k.dn1,
                ssig2: sbet2,
                csig2: cbet2,
                dn2: k.dn2,
            };
            let l = lengths(self.n, PI + bet12a, &e, REDUCED_LENGTH);
            x = -1.0 + l.m12b / (cbet1 * cbet2 * l.m0 * PI);
            let betscale = if x < -0.01 {
                sbet12a / x
            } else {
                -self.f * sq(cbet1) * PI
            };
            lamscale = betscale / cbet1;
            y = lam12x / lamscale;
        }
        if y > -self.tol1 && x > -1.0 - self.xthresh {
            // Strip near the cut.
            if self.f >= 0.0 {
                let salp1 = cmin(1.0, -x);
                (salp1, -libm::sqrt(1.0 - sq(salp1)))
            } else {
                let calp1 = cmax(if x > -self.tol1 { 0.0 } else { -1.0 }, x);
                (libm::sqrt(1.0 - sq(calp1)), calp1)
            }
        } else {
            // omg12 from the astroid, via omg12a = pi - omg12 since omg12 is
            // near pi, then the spherical formula for alpha1.
            let kk = astroid(x, y);
            let omg12a = lamscale
                * if self.f >= 0.0 {
                    -x * kk / (1.0 + kk)
                } else {
                    -y * (1.0 + kk) / kk
                };
            let somg12 = libm::sin(omg12a);
            let comg12 = -libm::cos(omg12a);
            (
                cbet2 * somg12,
                sbet12a - cbet2 * sbet1 * sq(somg12) / (1.0 - comg12),
            )
        }
    }

    /// `Lambda12` at alpha1 = (salp1, calp1), with its derivative when
    /// `diffp`.
    pub(super) fn lambda12(&self, k: &Canonical, salp1: f64, calp1: f64, diffp: bool) -> Lambda {
        let (sbet1, cbet1, sbet2, cbet2) = (k.sbet1, k.cbet1, k.sbet2, k.cbet2);
        // Break the degeneracy of the equatorial line (handled already).
        let calp1 = if sbet1 == 0.0 && calp1 == 0.0 {
            -self.tiny
        } else {
            calp1
        };
        // sin(alp1) cos(bet1) = sin(alp0).
        let salp0 = salp1 * cbet1;
        let calp0 = libm::hypot(calp1, salp1 * sbet1);
        // tan(bet1) = tan(sig1) cos(alp1); tan(omg1) = sin(alp0) tan(sig1).
        let somg1 = salp0 * sbet1;
        let comg1 = calp1 * cbet1;
        let (ssig1, csig1) = norm(sbet1, comg1);
        // Symmetries where |bet2| = -bet1, which can make the iteration
        // singular.
        let salp2 = if cbet2 != cbet1 { salp0 / cbet2 } else { salp1 };
        let calp2 = if cbet2 != cbet1 || sbet2.abs() != -sbet1 {
            libm::sqrt(
                sq(calp1 * cbet1)
                    + if cbet1 < -sbet1 {
                        (cbet2 - cbet1) * (cbet1 + cbet2)
                    } else {
                        (sbet1 - sbet2) * (sbet1 + sbet2)
                    },
            ) / cbet2
        } else {
            calp1.abs()
        };
        let somg2 = salp0 * sbet2;
        let comg2 = calp2 * cbet2;
        let (ssig2, csig2) = norm(sbet2, comg2);
        // sig12 = sig2 - sig1 and omg12 = omg2 - omg1, limited to [0, pi].
        let sig12 = libm::atan2(
            cmax(0.0, csig1 * ssig2 - ssig1 * csig2),
            csig1 * csig2 + ssig1 * ssig2,
        );
        let somg12 = cmax(0.0, comg1 * somg2 - somg1 * comg2);
        let comg12 = comg1 * comg2 + somg1 * somg2;
        // eta = omg12 - lam120.
        let eta = libm::atan2(
            somg12 * k.clam12 - comg12 * k.slam12,
            comg12 * k.clam12 + somg12 * k.slam12,
        );
        let k2 = sq(calp0) * self.ep2;
        let eps = k2 / (2.0 * (1.0 + libm::sqrt(1.0 + k2)) + k2);
        let ca = self.c3x.evaluate(eps);
        let b312 =
            sin_cos_series(true, ssig2, csig2, &ca) - sin_cos_series(true, ssig1, csig1, &ca);
        let domg12 = -self.f * self.a3f(eps) * salp0 * (sig12 + b312);
        let ends = Ends {
            ssig1,
            csig1,
            dn1: k.dn1,
            ssig2,
            csig2,
            dn2: k.dn2,
        };
        let mut dlam12 = f64::NAN;
        if diffp {
            dlam12 = if calp2 == 0.0 {
                -2.0 * self.f1 * k.dn1 / sbet1
            } else {
                lengths(eps, sig12, &ends, REDUCED_LENGTH).m12b * (self.f1 / (calp2 * cbet2))
            };
        }
        Lambda {
            lam12: eta + domg12,
            salp2,
            calp2,
            sig12,
            ends,
            eps,
            dlam12,
        }
    }
}
