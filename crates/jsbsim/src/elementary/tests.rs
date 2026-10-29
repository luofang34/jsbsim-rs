// SPDX-License-Identifier: LGPL-2.1-or-later
//! Bit pins of the elementary functions over their edge cases, and sweeps
//! that hold them within an ulp of the `libm` crate.

use std::hint::black_box;

use super::exp;

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

#[test]
fn exp_is_the_same_bits_on_every_target() {
    for (x, want) in EXP_PINS {
        let x = f64::from_bits(x);
        let got = exp(black_box(x)).to_bits();
        assert_eq!(got, want, "exp({x:e}) = {got:#018x}, pinned {want:#018x}");
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
