// SPDX-License-Identifier: LGPL-2.1-or-later
//! Bit pins of the elementary functions over their edge cases, and sweeps
//! that hold them within an ulp of the `libm` crate.

use std::hint::black_box;

use super::{exp, pow};

/// Bits of `x` and `exp(x)`. Pinned because every target must compute
/// the same bits; a target that differs fails here. Against 80-digit
/// references the errors are at most 0.43 ulp, 0.5 where the result
/// rounds into the subnormals or to zero.
const EXP_PINS: [(u64, u64); 43] = [
    (0x0000000000000000, 0x3ff0000000000000), // +0
    (0x8000000000000000, 0x3ff0000000000000), // -0
    (0x7ff0000000000000, 0x7ff0000000000000), // +inf
    (0xfff0000000000000, 0x0000000000000000), // -inf
    (0x7ff8000000000000, 0x7ff8000000000000), // quiet NaN
    (0x7ff800000000beef, 0x7ff800000000beef), // quiet NaN with payload
    (0x7ff000000000beef, 0x7ff800000000beef), // signaling NaN with payload
    (0xfff8000000000123, 0xfff8000000000123), // negative NaN with payload
    (0x0000000000000001, 0x3ff0000000000000), // subnormal in
    (0x800fffffffffffff, 0x3ff0000000000000), // negative subnormal in
    (0x3c90000000000000, 0x3ff0000000000000), // tiny threshold 2^-54
    (0x3c8fffffffffffff, 0x3ff0000000000000), // just below 2^-54
    (0xbc30000000000000, 0x3ff0000000000000), // tiny negative
    (0x3ff0000000000000, 0x4005bf0a8b145769), // 1
    (0xbff0000000000000, 0x3fd78b56362cef38), // -1
    (0x3fe0000000000000, 0x3ffa61298e1e069c), // 0.5
    (0x3ddb7cdfd9d7bdbb, 0x3ff000000006df38), // near 1
    (0xbe45798ee2308c3a, 0x3feffffffaa19c48), // near 1
    (0x4024000000000000, 0x40d5829dcf950560), // 10
    (0xc024000000000000, 0x3f07cd79b5647c9b), // -10
    (0x4059000000000000, 0x48f3494a9b171bf5), // 100
    (0x3fb999999999999a, 0x3ff1aec7b35a00d4), // 0.1
    (0x407fffffffffffff, 0x6e19476504ba839a), // just below 512 (fast path limit)
    (0x4080000000000000, 0x6e19476504ba852e), // 512 (slow path)
    (0x40862e42fefa39ef, 0x7fefffffffffff2a), // upper threshold: largest finite result
    (0x40862e42fefa39f0, 0x7ff0000000000000), // upper threshold: first overflow
    (0x4086280000000000, 0x7fdd422d2be5dc9b), // 709
    (0x4086300000000000, 0x7ff0000000000000), // 710 overflows
    (0x4090000000000000, 0x7ff0000000000000), // 1024
    (0xc090000000000000, 0x0000000000000000), // -1024
    (0xc086232bdd7abcd2, 0x001000000000007c), // lower threshold of normal results
    (0xc086232bdd7abcd3, 0x000ffffffffffe7c), // just below the normal range
    (0xc086280000000000, 0x0008bfe55de02338), // -709, subnormal out
    (0xc087200000000000, 0x0000000000000055), // -740, subnormal out
    (0xc0874385446d71c3, 0x0000000000000001), // near the smallest subnormal result
    (0xc0874910d52d3051, 0x0000000000000001), // lower threshold: last nonzero result
    (0xc0874910d52d3052, 0x0000000000000000), // lower threshold: first zero result
    (0xc087500000000000, 0x0000000000000000), // -746 underflows
    (0xfe37e43c8800759c, 0x0000000000000000), // large negative
    (0x7e37e43c8800759c, 0x7ff0000000000000), // large positive
    (0xbfe62e42fefa39ef, 0x3fe0000000000000), // -ln2
    (0x3c814b37f4b51f71, 0x3ff0000000000000), // below 2^-54
    (0x81c01297d23ab683, 0x3ff0000000000000), // tiny negative normal
];

/// Bits of `x`, `y` and `pow(x, y)`, pinned as `EXP_PINS` are; at most
/// 0.4985 ulp from 80-digit references, 0.5 where the result rounds into
/// the subnormals or to zero.
const POW_PINS: [(u64, u64, u64); 94] = [
    (0x4000000000000000, 0x3fe0000000000000, 0x3ff6a09e667f3bcd), // sqrt 2
    (0x4000000000000000, 0x4024000000000000, 0x4090000000000000), // 2^10
    (0x4024000000000000, 0xc008000000000000, 0x3f50624dd2f1a9fc), // 10^-3
    (0x3fe0000000000000, 0x3fd3333333333333, 0x3fe9fdf8bcce533e),
    (0x4072026666666666, 0x4015060aa64c2f83, 0x429ec9839a7e2c6d), // atmosphere-like
    (0x3feccccccccccccd, 0xbff6666666666666, 0x3ff28b035dc6316f),
    (0x4008000000000000, 0x3fd5555555555555, 0x3ff7137449123ef6),
    (0x7ff8000000000000, 0x0000000000000000, 0x3ff0000000000000), // pow(NaN, 0)
    (0x7ff800000000beef, 0x8000000000000000, 0x3ff0000000000000), // pow(NaN payload, -0)
    (0x7ff000000000beef, 0x0000000000000000, 0x7ff800000000beef), // pow(sNaN, 0) is NaN
    (0x3ff0000000000000, 0x7ff8000000000000, 0x3ff0000000000000), // pow(1, NaN)
    (0x3ff0000000000000, 0x7ff800000000beef, 0x3ff0000000000000), // pow(1, NaN payload)
    (0x3ff0000000000000, 0x7ff000000000beef, 0x7ff800000000beef), // pow(1, sNaN) is NaN
    (0x3ff0000000000000, 0x7ff0000000000000, 0x3ff0000000000000), // pow(1, inf)
    (0xbff0000000000000, 0x7ff0000000000000, 0x3ff0000000000000), // pow(-1, inf)
    (0xbff0000000000000, 0xfff0000000000000, 0x3ff0000000000000), // pow(-1, -inf)
    (0x7ff800000000beef, 0x4000000000000000, 0x7ff800000000beef), // NaN payload x
    (0x4000000000000000, 0x7ff800000000beef, 0x7ff800000000beef), // NaN payload y
    (0xfff8000000000123, 0x4008000000000000, 0xfff8000000000123), // negative NaN payload x, odd y
    (0x7ff800000000beef, 0xfff8000000000123, 0x7ff800000000beef), // two NaNs: x wins
    (0x7ff000000000beef, 0x4000000000000000, 0x7ff800000000beef), // sNaN x quieted
    (0x4000000000000000, 0x7ff000000000beef, 0x7ff800000000beef), // sNaN y quieted
    (0x0000000000000000, 0x4008000000000000, 0x0000000000000000), // +0^odd
    (0x8000000000000000, 0x4008000000000000, 0x8000000000000000), // -0^odd
    (0x8000000000000000, 0x4000000000000000, 0x0000000000000000), // -0^even
    (0x8000000000000000, 0x3fe0000000000000, 0x0000000000000000), // -0^0.5
    (0x0000000000000000, 0xbff0000000000000, 0x7ff0000000000000), // +0^-1
    (0x8000000000000000, 0xc008000000000000, 0xfff0000000000000), // -0^negative odd
    (0x8000000000000000, 0xc000000000000000, 0x7ff0000000000000), // -0^negative even
    (0x8000000000000000, 0xbfe0000000000000, 0x7ff0000000000000), // -0^-0.5
    (0x0000000000000000, 0xfff0000000000000, 0x7ff0000000000000), // 0^-inf
    (0x0000000000000000, 0x7ff0000000000000, 0x0000000000000000), // 0^inf
    (0x7ff0000000000000, 0x4000000000000000, 0x7ff0000000000000), // inf^2
    (0x7ff0000000000000, 0xc000000000000000, 0x0000000000000000), // inf^-2
    (0xfff0000000000000, 0x4008000000000000, 0xfff0000000000000), // -inf^odd
    (0xfff0000000000000, 0xc008000000000000, 0x8000000000000000), // -inf^negative odd
    (0xfff0000000000000, 0x4000000000000000, 0x7ff0000000000000), // -inf^even
    (0xfff0000000000000, 0xc000000000000000, 0x0000000000000000), // -inf^negative even
    (0xfff0000000000000, 0x3fe0000000000000, 0x7ff0000000000000), // -inf^0.5
    (0x4000000000000000, 0x7ff0000000000000, 0x7ff0000000000000), // 2^inf
    (0x3fe0000000000000, 0x7ff0000000000000, 0x0000000000000000), // 0.5^inf
    (0x4000000000000000, 0xfff0000000000000, 0x0000000000000000), // 2^-inf
    (0x3fe0000000000000, 0xfff0000000000000, 0x7ff0000000000000), // 0.5^-inf
    (0xc000000000000000, 0x4008000000000000, 0xc020000000000000), // negative x, odd y
    (0xc000000000000000, 0x4010000000000000, 0x4030000000000000), // negative x, even y
    (0xc000000000000000, 0xc008000000000000, 0xbfc0000000000000), // negative x, negative odd y
    (0xbff8000000000000, 0x4330000000000001, 0xfff0000000000000), // negative x, odd y 2^52+1
    (0xbff000001ad7f29b, 0x4340000000000000, 0x7ff0000000000000), // negative x, even y 2^53
    (0xc000000000000000, 0x3fe0000000000000, 0x7ff8000000000000), // negative x, non-integer y
    (0xc020000000000000, 0x3fd5555555555555, 0x7ff8000000000000), // negative x, non-integer y
    (0xbfe0000000000000, 0x3ff8000000000000, 0x7ff8000000000000), // negative x, non-integer y
    (0xc000000000000000, 0x39b4484bfeebc2a0, 0x7ff8000000000000), // negative x, tiny y
    (0xc000000000000000, 0x7e37e43c8800759c, 0x7ff0000000000000), // negative x, huge even y
    (0xbfe0000000000000, 0x7e37e43c8800759c, 0x0000000000000000), // negative x, huge y, |x| < 1
    (0x4000000000000000, 0x7e37e43c8800759c, 0x7ff0000000000000), // huge y
    (0x3fe0000000000000, 0x7e37e43c8800759c, 0x0000000000000000), // huge y, x < 1
    (0x3ff8000000000000, 0x4415af1d78b58c40, 0x7ff0000000000000), // large y
    (0x3ff0000000000001, 0x43d0000000000000, 0x7ff0000000000000), // near 1, large y
    (0x3fefffffffffffff, 0x43e0000000000000, 0x0000000000000000), // near 1, y 2^63
    (0x3ff0000000000001, 0x430c6bf526340000, 0x3ff3fa60615291ee), // near 1, large y
    (0x3feffffffffffffe, 0x3fe0000000000000, 0x3fefffffffffffff), // near 1
    (0x3ff000000006df38, 0x4008000000000000, 0x3ff0000000149da8), // near 1
    (0x3fefffffca501acb, 0xc16312d000000000, 0x4005bf0a9d212735), // near 1, large negative y
    (0x3ff000001ad7f29b, 0x416312d000000000, 0x4005bf0a790ce6f2), // near 1, large y
    (0x4000000000000000, 0x3bc79ca10c924223, 0x3ff0000000000000), // |y| < 2^-65, x > 1
    (0x3fe0000000000000, 0x39b4484bfeebc2a0, 0x3ff0000000000000), // |y| < 2^-65, x < 1
    (0x4000000000000000, 0x408ff80000000000, 0x7fe0000000000000), // largest power of 2
    (0x4000000000000000, 0x4090000000000000, 0x7ff0000000000000), // overflow threshold
    (0x4000000000000000, 0x408fffffffffffff, 0x7feffffffffffd3a), // just below overflow
    (0x7fefffffffffffff, 0x3ff0000000000000, 0x7fefffffffffffff), // DBL_MAX^1
    (0x7fefffffffffffff, 0x3ff0000000000001, 0x7ff0000000000000), // just above DBL_MAX
    (0x4024000000000000, 0x4073440000000000, 0x7fefa788589d81d3),
    (0x4024000000000000, 0x40734428f5c28f5c, 0x7ff0000000000000), // overflows
    (0x4000000000000000, 0xc08ff00000000000, 0x0010000000000000), // smallest normal
    (0x4000000000000000, 0xc08ffc0000000000, 0x0005a827999fcef3), // subnormal out
    (0x4000000000000000, 0xc090c80000000000, 0x0000000000000001), // smallest subnormal
    (0x4000000000000000, 0xc090ca0000000000, 0x0000000000000001), // rounds to the smallest subnormal or 0
    (0x4000000000000000, 0xc090cc0000000000, 0x0000000000000000), // underflow tie to 0
    (0x3fe0000000000000, 0x4090c80000000000, 0x0000000000000001), // smallest subnormal from x < 1
    (0x4024000000000000, 0xc07434cccccccccd, 0x0000000000000001), // subnormal out
    (0x4024000000000000, 0xc074400000000000, 0x0000000000000000), // underflows
    (0xc000000000000000, 0x4090040000000000, 0xfff0000000000000), // negative overflow
    (0xc000000000000000, 0xc090cc0000000000, 0x8000000000000000), // negative underflow to -0
    (0xc000000000000000, 0xc090c40000000000, 0x8000000000000002), // negative subnormal out
    (0x0000000000000001, 0x3fe0000000000000, 0x1e60000000000000), // subnormal x
    (0x000fffffffffffff, 0x3ff0000000000000, 0x000fffffffffffff), // largest subnormal x
    (0x0000000000000001, 0xbff0000000000000, 0x7ff0000000000000), // subnormal x overflows
    (0x8000000000000001, 0x4008000000000000, 0x8000000000000000), // negative subnormal x, odd y
    (0x000012688b70e62b, 0x3fb999999999999a, 0x398039d66589686c), // subnormal x
    (0x0000000000000001, 0x3ff0000000000000, 0x0000000000000001), // subnormal^1
    (0x16687e92154ef7ac, 0x3ff999999999999a, 0x00000000000007e8), // subnormal out
    (0x0010000000000000, 0x3feff7ced916872b, 0x00203ef0864b3848),
    (0x401c000000000000, 0x0000000000000000, 0x3ff0000000000000), // y = 0
    (0xc01c000000000000, 0x8000000000000000, 0x3ff0000000000000), // y = -0
];

#[test]
fn exp_is_the_same_bits_on_every_target() {
    for (x, want) in EXP_PINS {
        let x = f64::from_bits(x);
        let got = exp(black_box(x)).to_bits();
        assert_eq!(got, want, "exp({x:e}) = {got:#018x}, pinned {want:#018x}");
    }
}

#[test]
fn pow_is_the_same_bits_on_every_target() {
    for (x, y, want) in POW_PINS {
        let (x, y) = (f64::from_bits(x), f64::from_bits(y));
        let got = pow(black_box(x), black_box(y)).to_bits();
        assert_eq!(
            got, want,
            "pow({x:e}, {y:e}) = {got:#018x}, pinned {want:#018x}"
        );
    }
}

/// Bits of `x`, `y`, `pow(x, y)` and Apple libm's `pow(x, y)` where the
/// two differ: Apple's returns 1 for a signaling NaN under a zero power or
/// over a base of 1, where optimized-routines quiets it, and of two NaNs
/// keeps `y`'s payload where `pow` keeps `x`'s. No golden case reaches
/// these inputs, so the port's results stand as pinned.
const POW_DEPARTURES_FROM_APPLE: [(u64, u64, u64, u64); 6] = [
    (
        0x7ff000000000beef,
        0x0000000000000000,
        0x7ff800000000beef,
        0x3ff0000000000000,
    ), // pow(sNaN, 0)
    (
        0x3ff0000000000000,
        0x7ff000000000beef,
        0x7ff800000000beef,
        0x3ff0000000000000,
    ), // pow(1, sNaN)
    (
        0x7ff800000000beef,
        0xfff8000000000123,
        0x7ff800000000beef,
        0xfff8000000000123,
    ), // two NaNs
    (
        0xfff8000000000123,
        0x7ff800000000beef,
        0xfff8000000000123,
        0x7ff800000000beef,
    ), // two NaNs
    (
        0x7ff000000000beef,
        0x7ff8000000000123,
        0x7ff800000000beef,
        0x7ff8000000000123,
    ), // sNaN, NaN
    (
        0x7ff800000000beef,
        0x7ff0000000000123,
        0x7ff800000000beef,
        0x7ff8000000000123,
    ), // NaN, sNaN
];

#[test]
fn pow_departs_from_apple_libm_as_pinned() {
    for (x, y, want, apple) in POW_DEPARTURES_FROM_APPLE {
        assert_ne!(want, apple, "{x:#018x}, {y:#018x} is not a departure");
        let (x, y) = (f64::from_bits(x), f64::from_bits(y));
        let got = pow(black_box(x), black_box(y)).to_bits();
        assert_eq!(
            got, want,
            "pow({x:e}, {y:e}) = {got:#018x}, pinned {want:#018x}"
        );
    }
}

/// On Apple targets `f64::powf` is Apple libm's `pow`; the departures
/// above are its answers there.
#[cfg(target_vendor = "apple")]
#[test]
fn apple_libm_answers_the_pinned_departures() {
    for (x, y, _, apple) in POW_DEPARTURES_FROM_APPLE {
        let (x, y) = (f64::from_bits(x), f64::from_bits(y));
        let got = black_box(x).powf(black_box(y)).to_bits();
        assert_eq!(
            got, apple,
            "Apple pow({x:e}, {y:e}) = {got:#018x}, pinned {apple:#018x}"
        );
    }
}

/// Distance in ulps between two finite doubles of the same sign.
fn ulps(a: f64, b: f64) -> u64 {
    a.to_bits().abs_diff(b.to_bits())
}

/// The `libm` crate is within 1 ulp and `exp` within 0.51, so they can
/// differ by at most 1 ulp; a mistyped table entry or coefficient puts
/// the inputs that use it further off.
#[test]
// Reason: the libm crate is the independent reference here.
#[allow(clippy::disallowed_methods)]
fn exp_stays_within_an_ulp_of_libm_over_every_table_entry() {
    // Steps of about ln2/128/7, so every entry is used many times.
    for i in 0..200_000_u32 {
        let x = -705.0 + f64::from(i) * 0.007_05;
        let (got, want) = (exp(x), libm::exp(x));
        assert!(ulps(got, want) <= 1, "exp({x:e}) = {got:e}, libm {want:e}");
    }
}

/// As for `exp`, over x spanning every entry of the logarithm's table in
/// both binades it covers, and y spanning the exponential's reduction.
#[test]
// Reason: the libm crate is the independent reference here.
#[allow(clippy::disallowed_methods)]
fn pow_stays_within_an_ulp_of_libm_over_every_table_entry() {
    for i in 0..40_000_u32 {
        let x = 0.25 + f64::from(i) * 0.000_093_75;
        for y in [0.3, -2.7, 5.2559, 17.0, -150.5, 1e-3] {
            let (got, want) = (pow(x, y), libm::pow(x, y));
            assert!(
                ulps(got, want) <= 1,
                "pow({x:e}, {y:e}) = {got:e}, libm {want:e}"
            );
        }
    }
}
