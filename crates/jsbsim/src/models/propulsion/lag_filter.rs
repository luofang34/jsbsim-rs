// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGJSBBase.h (Filter).
//
//! The first-order lag (`FGJSBBase::Filter`) that smooths the rotor's
//! height above ground and its transmission's free-wheel coupling.

/// `FGJSBBase::Filter`: a first-order low-pass filter discretised with
/// the bilinear transform at a fixed step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LagFilter {
    prev_in: f64,
    prev_out: f64,
    ca: f64,
    cb: f64,
}

impl LagFilter {
    /// `Filter(coeff, dt)`: break frequency `coeff`, rad/s, at step `dt`.
    #[must_use]
    pub fn new(coeff: f64, dt: f64) -> Self {
        let denom = 2.0 + coeff * dt;
        Self {
            prev_in: 0.0,
            prev_out: 0.0,
            ca: coeff * dt / denom,
            cb: (2.0 - coeff * dt) / denom,
        }
    }

    /// `execute(in)`: the filtered value of `input`.
    pub fn execute(&mut self, input: f64) -> f64 {
        let out = (input + self.prev_in) * self.ca + self.prev_out * self.cb;
        self.prev_in = input;
        self.prev_out = out;
        out
    }

    /// `reset`.
    pub fn reset(&mut self) {
        self.prev_in = 0.0;
        self.prev_out = 0.0;
    }
}
