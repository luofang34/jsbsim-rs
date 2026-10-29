// SPDX-License-Identifier: LGPL-2.1-or-later AND (MIT OR Apache-2.0 WITH LLVM-exception)
// Derived from ARM optimized-routines math/pow.c, math/pow_common.h,
// math/math_err.c and math/math_config.h at
// commit 2ee4b0d45680a9880412c1d186224b08652c35f6
// (https://github.com/ARM-software/optimized-routines): Copyright (c)
// 2018-2026, Arm Limited, licensed under MIT OR Apache-2.0 WITH
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
//! x^y, as optimized-routines computes it without FMA and without rounding
//! intrinsics: log(x) to about 68 bits as a hi + tail pair from a 128-entry
//! table and a degree-8 polynomial, y log(x) split exactly into hi and lo
//! parts, then e^(hi + lo) through `exp`'s reduction and table. Worst-case
//! error 0.54 ulp.

use core::ops::ControlFlow;

use super::exp::{TOP_512, TOP_1024, TOP_TINY, reduce, scale_and_tail};
use super::pow_log_data::{A, LN2_HI, LN2_LO, TABLE_BITS, log_entry};
use super::{pow2, quiet, top12};

/// Start of the subinterval [OFF, 2 OFF) that x is reduced into.
const OFF: u64 = 0x3fe6_9555_0000_0000;
/// Added to k, it flips the scale's sign bit: the result is negative.
const SIGN_BIAS: u64 = 0x800 << super::exp_data::TABLE_BITS;
const ONE: u64 = 0x3ff0_0000_0000_0000;
const INF: u64 = 0x7ff0_0000_0000_0000;
const SIGN: u64 = 0x8000_0000_0000_0000;
/// The NaN of an invalid operation: positive and quiet, with no payload,
/// where `(x - x) / (x - x)` would give each target's default NaN.
const INVALID: f64 = f64::from_bits(0x7ff8_0000_0000_0000);

/// x^y.
pub(super) fn pow(x: f64, y: f64) -> f64 {
    let (ix, sign_bias) = match special(x, y) {
        ControlFlow::Break(result) => return result,
        ControlFlow::Continue(reduced) => reduced,
    };
    let (hi, lo) = log_inline(ix);
    // Split y and log(x) so that yhi * lhi is exact.
    let iy = y.to_bits();
    let yhi = f64::from_bits(iy & (u64::MAX << 27));
    let ylo = y - yhi;
    let lhi = f64::from_bits(hi.to_bits() & (u64::MAX << 27));
    let llo = hi - lhi + lo;
    let ehi = yhi * lhi;
    // |elo| < |ehi| * 2^-25.
    let elo = ylo * lhi + y * llo;
    exp_inline(ehi, elo, sign_bias)
}

/// The result when x is not positive finite normal, or |y| is below 2^-65
/// or at least 2^63, or either is zero, infinite or NaN; otherwise the
/// bits of |x| (a subnormal normalized with a negative exponent) and the
/// sign bias of the result.
fn special(x: f64, y: f64) -> ControlFlow<f64, (u64, u64)> {
    let (mut ix, iy) = (x.to_bits(), y.to_bits());
    let (mut topx, topy) = (top12(x), top12(y));
    let y_out_of_range = (topy & 0x7ff).wrapping_sub(0x3be) >= 0x43e - 0x3be;
    if topx.wrapping_sub(0x001) < 0x7ff - 0x001 && !y_out_of_range {
        return ControlFlow::Continue((ix, 0));
    }
    if zero_inf_nan(iy) {
        return ControlFlow::Break(special_y(x, y));
    }
    if zero_inf_nan(ix) {
        if x.is_nan() {
            return ControlFlow::Break(quiet(x));
        }
        let mut x2 = x * x;
        if ix >> 63 != 0 && check_int(iy) == 1 {
            x2 = -x2;
        }
        return ControlFlow::Break(if iy >> 63 != 0 { 1.0 / x2 } else { x2 });
    }
    // Here x and y are non-zero finite.
    let mut sign_bias = 0;
    if ix >> 63 != 0 {
        match check_int(iy) {
            0 => return ControlFlow::Break(INVALID),
            1 => sign_bias = SIGN_BIAS,
            _ => {}
        }
        ix &= !SIGN;
        topx &= 0x7ff;
    }
    if y_out_of_range {
        // sign_bias == 0 here, because y is not odd.
        if ix == ONE {
            return ControlFlow::Break(1.0);
        }
        if (topy & 0x7ff) < 0x3be {
            // |y| < 2^-65, x^y ~= 1 + y log(x).
            return ControlFlow::Break(if ix > ONE { 1.0 + y } else { 1.0 - y });
        }
        let overflows = (ix > ONE) == (topy < 0x800);
        return ControlFlow::Break(if overflows { f64::INFINITY } else { 0.0 });
    }
    if topx == 0 {
        // Normalize a subnormal x so that its exponent becomes negative.
        ix = (x * pow2(52)).to_bits() & !SIGN;
        ix = ix.wrapping_sub(52 << 52);
    }
    ControlFlow::Continue((ix, sign_bias))
}

/// x^y for y zero, infinite or NaN.
fn special_y(x: f64, y: f64) -> f64 {
    let (ix, iy) = (x.to_bits(), y.to_bits());
    if iy << 1 == 0 {
        return if is_signaling(x) { nan_of(x, y) } else { 1.0 };
    }
    if ix == ONE {
        return if is_signaling(y) { nan_of(x, y) } else { 1.0 };
    }
    if ix << 1 > INF << 1 || iy << 1 > INF << 1 {
        return nan_of(x, y);
    }
    if ix << 1 == ONE << 1 {
        return 1.0;
    }
    if (ix << 1 < ONE << 1) == (iy >> 63 == 0) {
        // |x| < 1 and y == inf, or |x| > 1 and y == -inf.
        return 0.0;
    }
    y * y
}

/// The NaN `x + y` gives for a NaN operand: `x`'s payload if it is a NaN,
/// else `y`'s, quieted. Built from bits, because targets differ in which
/// operand's payload arithmetic keeps.
fn nan_of(x: f64, y: f64) -> f64 {
    quiet(if x.is_nan() { x } else { y })
}

/// Whether `x` is a signaling NaN (quiet bit clear).
fn is_signaling(x: f64) -> bool {
    (x.to_bits() ^ 0x0008_0000_0000_0000) << 1 > 0x7ff8_0000_0000_0000 << 1
}

/// Whether bits `i` are a zero, an infinity or a NaN.
fn zero_inf_nan(i: u64) -> bool {
    (i << 1).wrapping_sub(1) >= (INF << 1).wrapping_sub(1)
}

/// 0 if non-zero finite bits `iy` are not an integer, 1 if an odd one, 2
/// if an even one.
fn check_int(iy: u64) -> u8 {
    let e = (iy >> 52) & 0x7ff;
    if e < 0x3ff {
        return 0;
    }
    if e > 0x3ff + 52 {
        return 2;
    }
    let unit = 1u64 << (0x3ff + 52 - e);
    if iy & (unit - 1) != 0 {
        0
    } else if iy & unit != 0 {
        1
    } else {
        2
    }
}

/// log(x) for the bits `ix` of a positive x (a subnormal normalized with a
/// negative exponent), as a rounded `hi` and a `tail` holding about 15 more
/// bits.
fn log_inline(ix: u64) -> (f64, f64) {
    // x = 2^k z, z in [OFF, 2 OFF) and exact; z lies in subinterval i,
    // near whose centre c is.
    let tmp = ix.wrapping_sub(OFF);
    let i = (tmp >> (52 - TABLE_BITS)) % (1 << TABLE_BITS);
    let k = (tmp as i64) >> 52;
    let iz = ix.wrapping_sub(tmp & (0xfff << 52));
    let z = f64::from_bits(iz);
    let kd = k as f64;
    // log(x) = k ln2 + log(c) + log1p(z/c - 1).
    let [invc, logc, logctail] = log_entry(i);
    // Split z so that rhi, rlo and rhi * rhi are exact and |rlo| <= |r|;
    // 1/c has few bits, so z/c - 1 is exact.
    let zhi = f64::from_bits(iz.wrapping_add(1 << 31) & (u64::MAX << 32));
    let zlo = z - zhi;
    let rhi = zhi * invc - 1.0;
    let rlo = zlo * invc;
    let r = rhi + rlo;
    // k ln2 + log(c) + r.
    let t1 = kd * LN2_HI + logc;
    let t2 = t1 + r;
    let lo1 = kd * LN2_LO + logctail;
    let lo2 = t1 - t2 + r;
    // k ln2 + log(c) + r + A[0] r^2, A[0] = -0.5.
    let [a0, a1, a2, a3, a4, a5, a6] = A;
    let ar = a0 * r;
    let ar2 = r * ar;
    let ar3 = r * ar2;
    let arhi = a0 * rhi;
    let arhi2 = rhi * arhi;
    let hi = t2 + arhi2;
    let lo3 = rlo * (ar + arhi);
    let lo4 = t2 - hi + arhi2;
    // log1p(r) - r - A[0] r^2.
    let p = ar3 * (a1 + r * a2 + ar2 * (a3 + r * a4 + ar2 * (a5 + r * a6)));
    let lo = lo1 + lo2 + lo3 + lo4 + p;
    let y = hi + lo;
    (y, hi - y + lo)
}

/// sign e^(x + xtail), |xtail| < 2^-8/N and |xtail| <= |x|; the sign is
/// negative when `sign_bias` is `SIGN_BIAS`.
fn exp_inline(x: f64, xtail: f64, sign_bias: u64) -> f64 {
    let mut abstop = top12(x) & 0x7ff;
    if abstop.wrapping_sub(TOP_TINY) >= TOP_512.wrapping_sub(TOP_TINY) {
        if abstop.wrapping_sub(TOP_TINY) >= 0x8000_0000 {
            // |x| < 2^-54; avoids a spurious underflow for tiny x.
            let one = 1.0 + x;
            return if sign_bias != 0 { -one } else { one };
        }
        if abstop >= TOP_1024 {
            // x is finite: infinities and NaNs ended in `special`.
            let magnitude = if x.is_sign_negative() {
                0.0
            } else {
                f64::INFINITY
            };
            return if sign_bias != 0 {
                -magnitude
            } else {
                magnitude
            };
        }
        // Large |x| goes through `special_case` below.
        abstop = 0;
    }
    let (ki, r) = reduce(x);
    let (tmp, sbits) = scale_and_tail(ki, r + xtail, sign_bias);
    if abstop == 0 {
        return special_case(tmp, sbits, ki);
    }
    let scale = f64::from_bits(sbits);
    scale + scale * tmp
}

/// scale * (1 + tmp) for a signed scale whose exponent may have overflowed
/// (k > 0, by at most 460) or underflowed (k < 0), without intermediate
/// rounding.
fn special_case(tmp: f64, sbits: u64, ki: u64) -> f64 {
    if ki & 0x8000_0000 == 0 {
        let scale = f64::from_bits(sbits.wrapping_sub(1009 << 52));
        return (scale + scale * tmp) * pow2(1009);
    }
    let sbits = sbits.wrapping_add(1022 << 52);
    let scale = f64::from_bits(sbits);
    let mut y = scale + scale * tmp;
    if y.abs() < 1.0 {
        // Round y to double precision before scaling it into the subnormal
        // range, so the result is not rounded twice.
        let one = if y < 0.0 { -1.0 } else { 1.0 };
        let lo = scale - y + scale * tmp;
        let hi = one + y;
        let lo = one - hi + y + lo;
        y = (hi + lo) - one;
        if y == 0.0 {
            // The zero takes the scale's sign.
            y = f64::from_bits(sbits & SIGN);
        }
    }
    pow2(-1022) * y
}
