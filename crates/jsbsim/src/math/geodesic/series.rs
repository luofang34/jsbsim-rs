// SPDX-License-Identifier: LGPL-2.1-or-later AND MIT
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
//! The series of the geodesic integrals for
//! `GEOGRAPHICLIB_GEODESIC_ORDER` 6 (the order for doubles), whose
//! Maxima-generated coefficients are written out as exact quotients, and
//! `Lengths`.
//!
//! A Fourier coefficient array holds `C[1]` to `C[N]` in its slots 0 to
//! N - 1 (the C++ arrays leave slot 0 unused).

use super::angles::{polyval, sq};

/// Order of the expansions (`nA1_`, `nC1_`, `nA2_`, `nC2_`, `nA3_`, `nC3_`).
const ORDER: usize = 6;

/// `C1[1..=6]` or `C2[1..=6]`.
pub(super) type Fourier = [f64; ORDER];

/// `C3[1..=5]`.
pub(super) type Fourier3 = [f64; ORDER - 1];

/// `SinCosSeries`: Clenshaw summation of `sum(c[i] * sin(2 i x))` (`sinp`)
/// or `sum(c[i] * cos((2 i + 1) x))` over the coefficients `c`.
pub(super) fn sin_cos_series(sinp: bool, sinx: f64, cosx: f64, c: &[f64]) -> f64 {
    let ar = 2.0 * (cosx - sinx) * (cosx + sinx);
    let mut from_last = c.iter().rev();
    let mut y0 = if c.len() & 1 == 1 {
        from_last.next().copied().unwrap_or(0.0)
    } else {
        0.0
    };
    let mut y1 = 0.0;
    while let (Some(&first), Some(&second)) = (from_last.next(), from_last.next()) {
        y1 = ar * y0 - y1 + first;
        y0 = ar * y1 - y0 + second;
    }
    if sinp {
        2.0 * sinx * cosx * y0
    } else {
        cosx * (y0 - y1)
    }
}

/// `A1m1f`: the scale factor A1 - 1.
pub(super) fn a1m1f(eps: f64) -> f64 {
    let t = polyval(&[1.0, 4.0, 64.0, 0.0], sq(eps)) / 256.0;
    (t + eps) / (1.0 - eps)
}

/// `C1f`: the coefficients C1 of B1.
pub(super) fn c1f(eps: f64) -> Fourier {
    let eps2 = sq(eps);
    let mut d = eps;
    let mut term = |p: &[f64], divisor: f64| {
        let c = d * polyval(p, eps2) / divisor;
        d *= eps;
        c
    };
    [
        term(&[-1.0, 6.0, -16.0], 32.0),
        term(&[-9.0, 64.0, -128.0], 2048.0),
        term(&[9.0, -16.0], 768.0),
        term(&[3.0, -5.0], 512.0),
        term(&[-7.0], 1280.0),
        term(&[-7.0], 2048.0),
    ]
}

/// `A2m1f`: the scale factor A2 - 1.
pub(super) fn a2m1f(eps: f64) -> f64 {
    let t = polyval(&[-11.0, -28.0, -192.0, 0.0], sq(eps)) / 256.0;
    (t - eps) / (1.0 + eps)
}

/// `C2f`: the coefficients C2 of B2.
pub(super) fn c2f(eps: f64) -> Fourier {
    let eps2 = sq(eps);
    let mut d = eps;
    let mut term = |p: &[f64], divisor: f64| {
        let c = d * polyval(p, eps2) / divisor;
        d *= eps;
        c
    };
    [
        term(&[1.0, 2.0, 16.0], 32.0),
        term(&[35.0, 64.0, 384.0], 2048.0),
        term(&[15.0, 80.0], 768.0),
        term(&[7.0, 35.0], 512.0),
        term(&[63.0], 1280.0),
        term(&[77.0], 2048.0),
    ]
}

/// `_A3x` (from `A3coeff`): the coefficients of A3 in descending powers of
/// eps, each a polynomial in the third flattening `n`.
pub(super) fn a3_coefficients(n: f64) -> [f64; ORDER] {
    [
        polyval(&[-3.0], n) / 128.0,
        polyval(&[-2.0, -3.0], n) / 64.0,
        polyval(&[-1.0, -3.0, -1.0], n) / 16.0,
        polyval(&[3.0, -1.0, -2.0], n) / 8.0,
        polyval(&[1.0, -1.0], n) / 2.0,
        polyval(&[1.0], n) / 1.0,
    ]
}

/// `_C3x` (from `C3coeff`): for each `C3[l]`, the coefficients of its
/// polynomial in eps (descending powers, the factor eps^l left out), each a
/// polynomial in `n`.
#[derive(Debug, Clone, Copy)]
pub(super) struct C3Coefficients {
    c1: [f64; 5],
    c2: [f64; 4],
    c3: [f64; 3],
    c4: [f64; 2],
    c5: [f64; 1],
}

impl C3Coefficients {
    pub(super) fn new(n: f64) -> Self {
        Self {
            c1: [
                polyval(&[3.0], n) / 128.0,
                polyval(&[2.0, 5.0], n) / 128.0,
                polyval(&[-1.0, 3.0, 3.0], n) / 64.0,
                polyval(&[-1.0, 0.0, 1.0], n) / 8.0,
                polyval(&[-1.0, 1.0], n) / 4.0,
            ],
            c2: [
                polyval(&[5.0], n) / 256.0,
                polyval(&[1.0, 3.0], n) / 128.0,
                polyval(&[-3.0, -2.0, 3.0], n) / 64.0,
                polyval(&[1.0, -3.0, 2.0], n) / 32.0,
            ],
            c3: [
                polyval(&[7.0], n) / 512.0,
                polyval(&[-10.0, 9.0], n) / 384.0,
                polyval(&[5.0, -9.0, 5.0], n) / 192.0,
            ],
            c4: [
                polyval(&[7.0], n) / 512.0,
                polyval(&[-14.0, 7.0], n) / 512.0,
            ],
            c5: [polyval(&[21.0], n) / 2560.0],
        }
    }

    /// `C3f`: the coefficients C3 of B3.
    pub(super) fn evaluate(&self, eps: f64) -> Fourier3 {
        let mut mult = 1.0;
        let mut term = |p: &[f64]| {
            mult *= eps;
            mult * polyval(p, eps)
        };
        [
            term(&self.c1),
            term(&self.c2),
            term(&self.c3),
            term(&self.c4),
            term(&self.c5),
        ]
    }
}

/// The ends of a geodesic on the auxiliary sphere, as `Lengths` takes them.
#[derive(Debug, Clone, Copy)]
pub(super) struct Ends {
    pub(super) ssig1: f64,
    pub(super) csig1: f64,
    pub(super) dn1: f64,
    pub(super) ssig2: f64,
    pub(super) csig2: f64,
    pub(super) dn2: f64,
}

/// What `Lengths` computes: the `DISTANCE` and `REDUCEDLENGTH` bits of its
/// output mask (`GEODESICSCALE` is never requested here).
#[derive(Debug, Clone, Copy)]
pub(super) struct LengthMask {
    pub(super) distance: bool,
    pub(super) reduced_length: bool,
}

/// The `REDUCEDLENGTH` mask alone.
pub(super) const REDUCED_LENGTH: LengthMask = LengthMask {
    distance: false,
    reduced_length: true,
};

/// `Lengths` results; NaN where not requested.
#[derive(Debug, Clone, Copy)]
pub(super) struct Lengths {
    /// Distance / b.
    pub(super) s12b: f64,
    /// Reduced length / b.
    pub(super) m12b: f64,
    /// Coefficient of the secular term of the reduced length.
    pub(super) m0: f64,
}

/// `Lengths(eps, sig12, ends, mask)`.
pub(super) fn lengths(eps: f64, sig12: f64, e: &Ends, mask: LengthMask) -> Lengths {
    let mut out = Lengths {
        s12b: f64::NAN,
        m12b: f64::NAN,
        m0: f64::NAN,
    };
    let (mut m0x, mut j12, mut a1, mut a2) = (0.0, 0.0, 0.0, 0.0);
    let (mut ca, mut cb): (Fourier, Fourier) = ([0.0; ORDER], [0.0; ORDER]);
    if mask.distance || mask.reduced_length {
        a1 = a1m1f(eps);
        ca = c1f(eps);
        if mask.reduced_length {
            a2 = a2m1f(eps);
            cb = c2f(eps);
            m0x = a1 - a2;
            a2 += 1.0;
        }
        a1 += 1.0;
    }
    if mask.distance {
        let b1 = sin_cos_series(true, e.ssig2, e.csig2, &ca)
            - sin_cos_series(true, e.ssig1, e.csig1, &ca);
        out.s12b = a1 * (sig12 + b1);
        if mask.reduced_length {
            let b2 = sin_cos_series(true, e.ssig2, e.csig2, &cb)
                - sin_cos_series(true, e.ssig1, e.csig1, &cb);
            j12 = m0x * sig12 + (a1 * b1 - a2 * b2);
        }
    } else if mask.reduced_length {
        for (b, a) in cb.iter_mut().zip(ca) {
            *b = a1 * a - a2 * *b;
        }
        j12 = m0x * sig12
            + (sin_cos_series(true, e.ssig2, e.csig2, &cb)
                - sin_cos_series(true, e.ssig1, e.csig1, &cb));
    }
    if mask.reduced_length {
        out.m0 = m0x;
        out.m12b =
            e.dn2 * (e.csig1 * e.ssig2) - e.dn1 * (e.ssig1 * e.csig2) - e.csig1 * e.csig2 * j12;
    }
    out
}
