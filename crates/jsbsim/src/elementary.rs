// SPDX-License-Identifier: LGPL-2.1-or-later
//! Elementary functions that compute the same bits on every target, wasm32
//! included, at the speed of the platform maths libraries: ports of ARM
//! optimized-routines' scalar double-precision routines on their paths
//! without FMA and without rounding intrinsics, so they need only IEEE
//! addition, multiplication and division, integer operations and table
//! lookups. Every NaN they return is built from bits, since targets differ
//! in the NaN that arithmetic on NaNs produces. The other functions the
//! port calls come from the `libm` crate.
//!
//! Under the `platform-libm` feature they forward to the `libm` crate
//! instead. scripts/oracle-sincos.sh enables it while it patches the
//! platform maths library in for `libm`, so that its comparison with C++
//! sees the same maths library on both sides.

#[cfg(not(feature = "platform-libm"))]
mod exp;
#[cfg(not(feature = "platform-libm"))]
mod exp_data;

#[cfg(all(test, not(feature = "platform-libm")))]
mod tests;

/// Whether these functions forward to the `libm` crate, under the
/// `platform-libm` feature.
pub const FORWARDS_TO_LIBM: bool = cfg!(feature = "platform-libm");

/// e^x, within 0.511 ulp.
#[cfg(not(feature = "platform-libm"))]
#[inline]
#[must_use]
pub fn exp(x: f64) -> f64 {
    exp::exp(x)
}

/// e^x from the `libm` crate.
#[cfg(feature = "platform-libm")]
// Reason: forwarding to `libm` is what the platform-libm feature is for.
#[allow(clippy::disallowed_methods)]
#[inline]
#[must_use]
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}

/// Sign and exponent bits of `x`.
#[cfg(not(feature = "platform-libm"))]
const fn top12(x: f64) -> u32 {
    (x.to_bits() >> 52) as u32
}

/// 2^e for an exponent `e` of a normal double, in [-1022, 1023].
#[cfg(not(feature = "platform-libm"))]
const fn pow2(e: i32) -> f64 {
    f64::from_bits(((1023 + e) as u64) << 52)
}

/// NaN `x` with its quiet bit set, as IEEE arithmetic would propagate it.
#[cfg(not(feature = "platform-libm"))]
const fn quiet(x: f64) -> f64 {
    f64::from_bits(x.to_bits() | 0x0008_0000_0000_0000)
}
