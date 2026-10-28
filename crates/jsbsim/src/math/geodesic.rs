// SPDX-License-Identifier: LGPL-2.1-or-later
// Derived from GeographicLib src/Geodesic.{hpp,cpp}, as bundled with JSBSim
// in src/GeographicLib: Copyright (c) Charles Karney (2009-2021)
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
//! The inverse geodesic problem of `GeographicLib::Geodesic`, for
//! `real = double` and `GEOGRAPHICLIB_GEODESIC_ORDER` 6, the configuration
//! JSBSim builds. It computes in GeographicLib's operation order with
//! `libm`'s transcendental functions, so it matches the C++ library up to
//! the last-ulp differences between `libm` and the platform's maths
//! library. Only the outputs `FGLocation` asks for are ported: the distance
//! and the azimuths, not the reduced length, geodesic scales or area.

mod angles;
mod inverse;
mod series;
mod solve;

#[cfg(test)]
mod tests;

use series::C3Coefficients;

/// `maxit1_`: Newton iterations before bisection may take over.
const MAXIT1: u32 = 20;
/// `maxit2_`: `maxit1_ + Math::digits() + 10`.
const MAXIT2: u32 = MAXIT1 + f64::MANTISSA_DIGITS + 10;

/// `GeographicLib::Geodesic`: an ellipsoid of revolution and the constants
/// its inverse problem needs.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Geodesic {
    tiny: f64,
    tol0: f64,
    tol1: f64,
    tolb: f64,
    xthresh: f64,
    a: f64,
    f: f64,
    f1: f64,
    ep2: f64,
    n: f64,
    b: f64,
    etol2: f64,
    a3x: [f64; 6],
    c3x: C3Coefficients,
}

impl Geodesic {
    /// `Geodesic(a, f)`: equatorial radius `a` and flattening `f`. The C++
    /// constructor throws for a non-positive or non-finite `a` or `b`; this
    /// one leaves such an ellipsoid to give NaN.
    pub(crate) fn new(a: f64, f: f64) -> Self {
        let tiny = libm::sqrt(f64::MIN_POSITIVE);
        let tol0 = f64::EPSILON;
        let tol2 = libm::sqrt(tol0);
        let f1 = 1.0 - f;
        let e2 = f * (2.0 - f);
        let n = f / (2.0 - f);
        Self {
            tiny,
            tol0,
            tol1: 200.0 * tol0,
            tolb: tol0 * tol2,
            xthresh: 1000.0 * tol2,
            a,
            f,
            f1,
            ep2: e2 / angles::sq(f1),
            n,
            b: a * f1,
            etol2: 0.1 * tol2
                / libm::sqrt(angles::cmax(0.001, f.abs()) * angles::cmin(1.0, 1.0 - f / 2.0) / 2.0),
            a3x: series::a3_coefficients(n),
            c3x: C3Coefficients::new(n),
        }
    }

    /// `Inverse(lat1, lon1, lat2, lon2, s12)`: the distance between two
    /// points given in degrees, in the unit of `a`.
    pub(crate) fn inverse_distance(&self, lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        self.gen_inverse(lat1, lon1, lat2, lon2, true).s12
    }

    /// `Inverse(lat1, lon1, lat2, lon2, azi1, azi2)`: the azimuths, degrees,
    /// at both ends of the geodesic between two points given in degrees.
    pub(crate) fn inverse_azimuths(
        &self,
        lat1: f64,
        lon1: f64,
        lat2: f64,
        lon2: f64,
    ) -> (f64, f64) {
        let r = self.gen_inverse(lat1, lon1, lat2, lon2, false);
        (
            angles::atan2d(r.salp1, r.calp1),
            angles::atan2d(r.salp2, r.calp2),
        )
    }

    /// `A3f`.
    fn a3f(&self, eps: f64) -> f64 {
        angles::polyval(&self.a3x, eps)
    }
}
