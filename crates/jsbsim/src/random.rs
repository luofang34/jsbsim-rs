// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2001  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGJSBBase.h (RandomNumberGenerator). The engine
// and distributions follow the algorithms of the libc++ headers
// include/c++/v1/__random/{linear_congruential_engine,
// default_random_engine,generate_canonical,uniform_real_distribution,
// normal_distribution}.h of the Apple SDK.
//
//! JSBSim's random number generator: `std::default_random_engine` with a
//! `uniform_real_distribution<double>(-1, 1)` and a
//! `normal_distribution<double>(0, 1)`.
//!
//! The C++ standard fixes the engines but neither which engine
//! `default_random_engine` is nor how the distributions turn its output
//! into doubles. This module reproduces libc++, the standard library of
//! the Apple toolchain the oracle builds JSBSim with, so that a seed gives
//! the same doubles, bit for bit. libstdc++ differs in both respects: its
//! `default_random_engine` is `minstd_rand0` (multiplier 16807, not
//! 48271), and its `normal_distribution` caches and consumes its uniform
//! deviates differently; a JSBSim built against it draws other numbers.
//!
//! JSBSim's generator is shared through `std::shared_ptr`: the executive
//! owns one that the sensors, the winds and the functions without a
//! `seed` draw from in turn, and reseeding it (`simulation/randomseed`)
//! reseeds them all. [`SharedGenerator`] is that pointer; its state is
//! atomic so that a [`Fdm`](crate::fdm::Fdm) holding it stays `Send`.

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// `minstd_rand`'s modulus, 2^31 - 1.
const MODULUS: u32 = 2_147_483_647;
/// `minstd_rand`'s multiplier.
const MULTIPLIER: u32 = 48_271;
/// `minstd_rand::min()`: 1, since the increment is 0.
const ENGINE_MIN: u32 = 1;
/// `minstd_rand::max()`.
const ENGINE_MAX: u32 = MODULUS - 1;

/// `linear_congruential_engine::seed(s)` for a zero increment
/// (`seed(false_type, true_type, s)`): a seed that is a multiple of the
/// modulus would stall the engine at 0, so it becomes 1.
#[must_use]
pub fn engine_state(seed: u32) -> u32 {
    match seed % MODULUS {
        0 => 1,
        s => s,
    }
}

/// `linear_congruential_engine::operator()` of `minstd_rand` on a 32-bit
/// `uint_fast32_t`: `__lce_ta<48271, 0, 2^31-1, unsigned(-1), _LCE_Schrage>`,
/// Schrage's algorithm, which computes `48271 x mod (2^31 - 1)` without
/// overflowing 32 bits.
#[must_use]
pub fn engine_next(x: u32) -> u32 {
    const Q: u32 = MODULUS / MULTIPLIER;
    const R: u32 = MODULUS % MULTIPLIER;
    let t0 = MULTIPLIER.wrapping_mul(x % Q);
    let t1 = R.wrapping_mul(x / Q);
    let carry = if t0 < t1 { MODULUS } else { 0 };
    t0.wrapping_add(carry).wrapping_sub(t1)
}

/// `RandomNumberGenerator`: the engine's state and the normal
/// distribution's cached second deviate.
#[derive(Debug)]
pub struct RandomNumberGenerator {
    /// The engine's `__x_`.
    state: AtomicU32,
    /// `normal_distribution::__v_`, as bits.
    cached_normal: AtomicU64,
    /// `normal_distribution::__v_hot_`.
    has_cached_normal: AtomicBool,
}

impl Default for RandomNumberGenerator {
    /// `RandomNumberGenerator(0)`, the executive's generator before any
    /// `simulation/randomseed` is written.
    fn default() -> Self {
        Self::new(0)
    }
}

impl Clone for RandomNumberGenerator {
    /// A copy at the same point of the sequence, as C++'s implicit copy.
    fn clone(&self) -> Self {
        Self {
            state: AtomicU32::new(self.state.load(Ordering::Relaxed)),
            cached_normal: AtomicU64::new(self.cached_normal.load(Ordering::Relaxed)),
            has_cached_normal: AtomicBool::new(self.has_cached_normal.load(Ordering::Relaxed)),
        }
    }
}

impl RandomNumberGenerator {
    /// `RandomNumberGenerator(unsigned int seed)`.
    #[must_use]
    pub fn new(seed: u32) -> Self {
        Self {
            state: AtomicU32::new(engine_state(seed)),
            cached_normal: AtomicU64::new(0),
            has_cached_normal: AtomicBool::new(false),
        }
    }

    /// `seed(value)`: reseeds the engine and resets both distributions,
    /// which drops a cached normal deviate. `uniform_real_distribution`
    /// keeps no state, so its `reset` does nothing.
    pub fn seed(&self, value: u32) {
        self.state.store(engine_state(value), Ordering::Relaxed);
        self.has_cached_normal.store(false, Ordering::Relaxed);
    }

    /// One engine output, in `[1, 2^31 - 2]`.
    pub fn next_u32(&self) -> u32 {
        let x = engine_next(self.state.load(Ordering::Relaxed));
        self.state.store(x, Ordering::Relaxed);
        x
    }

    /// `generate_canonical<double, 53>`: `k = 53 / 30 + 1 = 2` engine
    /// outputs, since `__log2` of the engine's range `2^31 - 2` is 30,
    /// combined low part first into `sp / rp^2` with `rp = 2^31 - 2`. The
    /// division may round up to exactly 1, which libc++ does not correct.
    #[must_use]
    pub fn canonical(&self) -> f64 {
        let rp = f64::from(ENGINE_MAX - ENGINE_MIN) + 1.0;
        let mut sp = f64::from(self.next_u32() - ENGINE_MIN);
        let base = rp;
        sp += f64::from(self.next_u32() - ENGINE_MIN) * base;
        sp / (base * rp)
    }

    /// `uniform_real_distribution<double>(a, b)`: `(b - a) * canonical + a`.
    #[must_use]
    pub fn uniform_in(&self, a: f64, b: f64) -> f64 {
        (b - a) * self.canonical() + a
    }

    /// `GetUniformRandomNumber`: uniform over `[-1, 1)`.
    #[must_use]
    pub fn uniform(&self) -> f64 {
        self.uniform_in(-1.0, 1.0)
    }

    /// `GetNormalRandomNumber`: `normal_distribution<double>(0, 1)`,
    /// Marsaglia's polar method. Each accepted pair of uniform deviates
    /// gives two normal ones; the second is cached for the next call.
    #[must_use]
    pub fn normal(&self) -> f64 {
        if self.has_cached_normal.swap(false, Ordering::Relaxed) {
            return standard(f64::from_bits(self.cached_normal.load(Ordering::Relaxed)));
        }
        let (u, v, s) = loop {
            let u = self.uniform();
            let v = self.uniform();
            let s = u * u + v * v;
            if !(s > 1.0 || s == 0.0) {
                break (u, v, s);
            }
        };
        let fp = (-2.0 * libm::log(s) / s).sqrt();
        self.cached_normal
            .store((v * fp).to_bits(), Ordering::Relaxed);
        self.has_cached_normal.store(true, Ordering::Relaxed);
        standard(u * fp)
    }
}

/// `__up * stddev() + mean()` with JSBSim's stddev 1 and mean 0, which
/// turns a negative zero positive.
fn standard(up: f64) -> f64 {
    const STDDEV: f64 = 1.0;
    const MEAN: f64 = 0.0;
    up * STDDEV + MEAN
}

/// The executive's generator, shared as JSBSim shares it through
/// `std::shared_ptr`.
pub type SharedGenerator = Arc<RandomNumberGenerator>;
