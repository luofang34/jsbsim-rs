// SPDX-License-Identifier: LGPL-2.1-or-later
//! Forwarders to the C maths library. Each is an ordinary call, so neither
//! rustc nor LLVM treats it as a builtin it may fold or fuse: `sin` and
//! `cos` of one argument stay two calls, as in an oracle built with
//! `-fno-builtin-sin -fno-builtin-cos`.

// Reason: FFI into the platform maths library is the purpose of this crate.
#![allow(unsafe_code)]

// SAFETY: these are the C99 <math.h> declarations. The `safe` ones take and
// return plain doubles and ints and touch no memory; `remquo` and `modf`
// write through their pointer and stay `unsafe`.
unsafe extern "C" {
    /// The platform's `acos`.
    pub safe fn acos(x: f64) -> f64;
    /// The platform's `asin`.
    pub safe fn asin(x: f64) -> f64;
    /// The platform's `atan`.
    pub safe fn atan(x: f64) -> f64;
    /// The platform's `atan2`.
    pub safe fn atan2(y: f64, x: f64) -> f64;
    /// The platform's `cbrt`.
    pub safe fn cbrt(x: f64) -> f64;
    /// The platform's `ceil`.
    pub safe fn ceil(x: f64) -> f64;
    /// The platform's `cos`.
    pub safe fn cos(x: f64) -> f64;
    /// The platform's `exp`.
    pub safe fn exp(x: f64) -> f64;
    /// The platform's `floor`.
    pub safe fn floor(x: f64) -> f64;
    /// The platform's `fmod`.
    pub safe fn fmod(x: f64, y: f64) -> f64;
    /// The platform's `hypot`.
    pub safe fn hypot(x: f64, y: f64) -> f64;
    /// The platform's `log`.
    pub safe fn log(x: f64) -> f64;
    /// The platform's `log10`.
    pub safe fn log10(x: f64) -> f64;
    /// The platform's `pow`.
    pub safe fn pow(x: f64, y: f64) -> f64;
    /// The platform's `remainder`.
    pub safe fn remainder(x: f64, y: f64) -> f64;
    /// The platform's `round`.
    pub safe fn round(x: f64) -> f64;
    /// The platform's `scalbn`.
    pub safe fn scalbn(x: f64, n: i32) -> f64;
    /// The platform's `sin`.
    pub safe fn sin(x: f64) -> f64;
    /// The platform's `sqrt`.
    pub safe fn sqrt(x: f64) -> f64;
    /// The platform's `tan`.
    pub safe fn tan(x: f64) -> f64;
    #[link_name = "remquo"]
    fn c_remquo(x: f64, y: f64, quo: *mut i32) -> f64;
    #[link_name = "modf"]
    fn c_modf(x: f64, iptr: *mut f64) -> f64;
}

/// `remquo(x, y)`: the remainder and the low bits of the quotient, as
/// `libm::remquo` returns them.
#[must_use]
pub fn remquo(x: f64, y: f64) -> (f64, i32) {
    let mut quo = 0;
    // SAFETY: `quo` is a live, writable i32 for the duration of the call.
    let r = unsafe { c_remquo(x, y, &raw mut quo) };
    (r, quo)
}

/// `modf(x)`: the fractional and integral parts, as `libm::modf` returns
/// them.
#[must_use]
pub fn modf(x: f64) -> (f64, f64) {
    let mut int = 0.0;
    // SAFETY: `int` is a live, writable f64 for the duration of the call.
    let frac = unsafe { c_modf(x, &raw mut int) };
    (frac, int)
}

/// `sincos(x)`: the platform's `sin` and `cos`, called one after the other
/// as the oracle built without sin/cos builtins calls them, not its fused
/// `__sincos_stret`.
#[must_use]
pub fn sincos(x: f64) -> (f64, f64) {
    (sin(x), cos(x))
}
