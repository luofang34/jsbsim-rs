// SPDX-License-Identifier: LGPL-2.1-or-later AND (MIT OR Apache-2.0 WITH LLVM-exception)
// Derived from ARM optimized-routines math/exp.c, math/math_err.c and
// math/math_config.h at commit 2ee4b0d45680a9880412c1d186224b08652c35f6
// (https://github.com/ARM-software/optimized-routines): Copyright (c)
// 2018-2025, Arm Limited, licensed under MIT OR Apache-2.0 WITH
// LLVM-exception (LICENSES/optimized-routines.txt). This port takes it
// under the MIT License, whose notice follows.
//
//   MIT License
//   -----------
//
//   Copyright (c) 1999-2022, Arm Limited.
//
//   Permission is hereby granted, free of charge, to any person obtaining a copy
//   of this software and associated documentation files (the "Software"), to deal
//   in the Software without restriction, including without limitation the rights
//   to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//   copies of the Software, and to permit persons to whom the Software is
//   furnished to do so, subject to the following conditions:
//
//   The above copyright notice and this permission notice shall be included in all
//   copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//   AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//   OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//   SOFTWARE.
//
//! e^x, as optimized-routines computes it without FMA and without rounding
//! intrinsics (`exp_inline` with no tail): x = k ln2/N + r, with k rounded
//! by the shift trick, then 2^(k/N) from the table times a degree-5
//! polynomial in r. Worst-case error 0.511 ulp.

use super::exp_data::{
    C2, C3, C4, C5, INV_LN2_N, NEG_LN2_HI_N, NEG_LN2_LO_N, SHIFT, TABLE_BITS, scale_entry,
};
use super::{pow2, quiet, top12};

/// `top12(0x1p-54)`: below it e^x rounds to 1 + x.
pub(super) const TOP_TINY: u32 = top12(pow2(-54));
/// `top12(512.0)`: from it on, the result may leave the normal range.
pub(super) const TOP_512: u32 = top12(512.0);
/// `top12(1024.0)`: from it on, the result is 0 or infinite.
pub(super) const TOP_1024: u32 = top12(1024.0);

/// e^x.
pub(super) fn exp(x: f64) -> f64 {
    let mut abstop = top12(x) & 0x7ff;
    if abstop.wrapping_sub(TOP_TINY) >= TOP_512.wrapping_sub(TOP_TINY) {
        if abstop.wrapping_sub(TOP_TINY) >= 0x8000_0000 {
            // |x| < 2^-54; avoids a spurious underflow for tiny x.
            return 1.0 + x;
        }
        if abstop >= TOP_1024 {
            if x.to_bits() == f64::NEG_INFINITY.to_bits() {
                return 0.0;
            }
            if abstop >= top12(f64::INFINITY) {
                // +inf stays; a NaN comes back quieted with its payload,
                // built from bits so every target returns the same NaN.
                return if x.is_nan() { quiet(x) } else { x };
            }
            return if x.is_sign_negative() {
                0.0
            } else {
                f64::INFINITY
            };
        }
        // Large |x| goes through `special_case` below.
        abstop = 0;
    }
    let (ki, r) = reduce(x);
    let (tmp, sbits) = scale_and_tail(ki, r, 0);
    if abstop == 0 {
        return special_case(tmp, sbits, ki);
    }
    let scale = f64::from_bits(sbits);
    // tmp == 0 or |tmp| > 2^-200 and scale > 2^-739, so no spurious
    // underflow here even without FMA.
    scale + scale * tmp
}

/// x = k ln2/N + r with integer k and |r| <= ln2/2N: the bits of the
/// shift-rounded k (k in their low bits) and r.
pub(super) fn reduce(x: f64) -> (u64, f64) {
    let z = INV_LN2_N * x;
    // z - kd is in [-1, 1] in non-nearest rounding modes.
    let kd = z + SHIFT;
    let ki = kd.to_bits();
    let kd = kd - SHIFT;
    (ki, x + kd * NEG_LN2_HI_N + kd * NEG_LN2_LO_N)
}

/// e^r - 1 plus the table tail, and the bits of 2^(k/N)'s scale, so that
/// e^x ~= scale + scale * tmp. `sign_bias` (0 or `SIGN_BIAS`) carries a
/// negative sign into the scale's sign bit.
pub(super) fn scale_and_tail(ki: u64, r: f64, sign_bias: u64) -> (f64, u64) {
    let [tail, scale] = scale_entry(ki);
    let top = ki.wrapping_add(sign_bias) << (52 - TABLE_BITS);
    let tail = f64::from_bits(tail);
    // A valid scale only while -1023*N < k < 1024*N; `special_case`
    // repairs the exponent otherwise.
    let sbits = scale.wrapping_add(top);
    let r2 = r * r;
    // Without FMA the worst-case error is 0.25/N ulp larger.
    let tmp = tail + r + r2 * (C2 + r * C3) + r2 * r2 * (C4 + r * C5);
    (tmp, sbits)
}

/// scale * (1 + tmp) for a scale whose exponent may have overflowed (k > 0,
/// by at most 460) or underflowed (k < 0), without intermediate rounding.
fn special_case(tmp: f64, sbits: u64, ki: u64) -> f64 {
    if ki & 0x8000_0000 == 0 {
        let scale = f64::from_bits(sbits.wrapping_sub(1009 << 52));
        return pow2(1009) * (scale + scale * tmp);
    }
    let scale = f64::from_bits(sbits.wrapping_add(1022 << 52));
    let mut y = scale + scale * tmp;
    if y < 1.0 {
        // Round y to double precision before scaling it into the subnormal
        // range, so the result is not rounded twice.
        let lo = scale - y + scale * tmp;
        let hi = 1.0 + y;
        let lo = 1.0 - hi + y + lo;
        y = (hi + lo) - 1.0;
    }
    pow2(-1022) * y
}
