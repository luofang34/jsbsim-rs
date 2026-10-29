// SPDX-License-Identifier: LGPL-2.1-or-later AND MIT
// Derived from GeographicLib src/Math.{hpp,cpp}, as bundled with JSBSim in
// src/GeographicLib: Math.hpp Copyright (c) Charles Karney (2008-2020),
// Math.cpp Copyright (c) Charles Karney (2015-2021)
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
//! The `GeographicLib::Math` helpers the inverse problem uses, for
//! `real = double`, in the C++ operation order.

/// `Math::pi()`: `atan2(0, -1)`, which is the double nearest pi.
pub(super) const PI: f64 = core::f64::consts::PI;

/// `Math::degree()`: radians per degree.
pub(super) const DEGREE: f64 = PI / 180.0;

/// `Math::sq`.
pub(super) fn sq(x: f64) -> f64 {
    x * x
}

/// `std::max(a, b)`: `b` only when `a < b`, so NaN and signed zeros resolve
/// as in C++.
pub(super) fn cmax(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// `std::min(a, b)`: `b` only when `b < a`.
pub(super) fn cmin(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// `Math::norm`: (x, y) scaled to unit length.
pub(super) fn norm(x: f64, y: f64) -> (f64, f64) {
    let h = libm::hypot(x, y);
    (x / h, y / h)
}

/// `Math::sum`: the rounded sum and its exact error, `u + v = s + t`.
pub(super) fn sum(u: f64, v: f64) -> (f64, f64) {
    let s = u + v;
    let mut up = s - v;
    let mut vpp = s - up;
    up -= u;
    vpp -= v;
    (s, -(up + vpp))
}

/// `Math::polyval`: Horner evaluation of the polynomial whose coefficients
/// `p` are in descending powers of `x`.
pub(super) fn polyval(p: &[f64], x: f64) -> f64 {
    let mut coefficients = p.iter();
    let Some(&first) = coefficients.next() else {
        return 0.0;
    };
    coefficients.fold(first, |y, &c| y * x + c)
}

/// `Math::AngNormalize`: an angle in degrees reduced to (-180, 180].
pub(super) fn ang_normalize(x: f64) -> f64 {
    let x = libm::remainder(x, 360.0);
    if x != -180.0 { x } else { 180.0 }
}

/// `Math::LatFix`: NaN for a latitude beyond +/-90 degrees.
pub(super) fn lat_fix(x: f64) -> f64 {
    if x.abs() > 90.0 { f64::NAN } else { x }
}

/// `Math::AngDiff`: `y - x` reduced to [-180, 180], and its rounding error.
pub(super) fn ang_diff(x: f64, y: f64) -> (f64, f64) {
    let (d, t) = sum(libm::remainder(-x, 360.0), libm::remainder(y, 360.0));
    let d = ang_normalize(d);
    sum(if d == 180.0 && t > 0.0 { -180.0 } else { d }, t)
}

/// `Math::AngRound`: rounds tiny angles so that they are multiples of
/// 2^-57 degrees, and -0 to +0.
pub(super) fn ang_round(x: f64) -> f64 {
    const Z: f64 = 1.0 / 16.0;
    if x == 0.0 {
        return 0.0;
    }
    let y = x.abs();
    let y = if y < Z { Z - (Z - y) } else { y };
    if x < 0.0 { -y } else { y }
}

/// `Math::sincosd`: sine and cosine of an angle in degrees, reduced exactly
/// to [-45, 45] first.
pub(super) fn sincosd(x: f64) -> (f64, f64) {
    let (r, q) = libm::remquo(x, 90.0);
    let r = r * DEGREE;
    let (s, c) = libm::sincos(r);
    let (mut sinx, mut cosx) = match q.cast_unsigned() & 3 {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    };
    if x != 0.0 {
        sinx += 0.0;
        cosx += 0.0;
    }
    (sinx, cosx)
}

/// `Math::atan2d`: `atan2(y, x)` in degrees, computed in [-45, 45] and then
/// moved to its quadrant.
pub(super) fn atan2d(y: f64, x: f64) -> f64 {
    let (mut y, mut x) = (y, x);
    let mut q = 0;
    if y.abs() > x.abs() {
        core::mem::swap(&mut x, &mut y);
        q = 2;
    }
    if x < 0.0 {
        x = -x;
        q += 1;
    }
    let ang = libm::atan2(y, x) / DEGREE;
    match q {
        1 => (if y >= 0.0 { 180.0 } else { -180.0 }) - ang,
        2 => 90.0 - ang,
        3 => -90.0 + ang,
        _ => ang,
    }
}
