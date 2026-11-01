// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2009 James Goppert.
// Derived from JSBSim src/initialization/FGLinearization.{h,cpp}.
//
//! Linearization: the state-space model `dx/dt = A x + B u`,
//! `y = C x + D u` of the aircraft about its current (trimmed) state,
//! by central differences through the simulation (`FGStateSpace`).
//! [`Fdm::do_linearization`](crate::Fdm::do_linearization) computes it.
//!
//! JSBSim writes the matrices to a Scicoslab file; the core does no file
//! I/O and returns them instead. Writing the file changes nothing a
//! simulation can observe.

mod component;
mod matrix;
mod state_space;
#[cfg(test)]
mod tests;

pub use component::Component;
pub use matrix::Matrix;
pub(crate) use state_space::StateSpace;

/// What `FGLinearization` computes.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Linearization {
    /// The aircraft's name.
    pub aircraft_name: String,
    /// The states, in order.
    pub x: Vec<Component>,
    /// The inputs, in order.
    pub u: Vec<Component>,
    /// The outputs (the states: state feedback), in order.
    pub y: Vec<Component>,
    /// The states at the operating point.
    pub x0: Vec<f64>,
    /// The inputs at the operating point.
    pub u0: Vec<f64>,
    /// The outputs at the operating point.
    pub y0: Vec<f64>,
    /// `A = d(dx/dt)/dx`: a row per state and a column per state.
    pub a: Matrix,
    /// `B = d(dx/dt)/du`: a row per state and a column per input.
    pub b: Matrix,
    /// `C = dy/dx`: a row per output and a column per state.
    pub c: Matrix,
    /// `D = dy/du`: a row per output and a column per input.
    pub d: Matrix,
}

impl Linearization {
    /// The names of the states, in order (`x_names`).
    #[must_use]
    pub fn x_names(&self) -> Vec<&'static str> {
        self.x.iter().map(|c| c.name()).collect()
    }

    /// The names of the inputs, in order (`u_names`).
    #[must_use]
    pub fn u_names(&self) -> Vec<&'static str> {
        self.u.iter().map(|c| c.name()).collect()
    }

    /// The names of the outputs, in order (`y_names`).
    #[must_use]
    pub fn y_names(&self) -> Vec<&'static str> {
        self.y.iter().map(|c| c.name()).collect()
    }

    /// The units of the states, in order (`x_units`).
    #[must_use]
    pub fn x_units(&self) -> Vec<&'static str> {
        self.x.iter().map(|c| c.unit()).collect()
    }

    /// The units of the inputs, in order (`u_units`).
    #[must_use]
    pub fn u_units(&self) -> Vec<&'static str> {
        self.u.iter().map(|c| c.unit()).collect()
    }

    /// The units of the outputs, in order (`y_units`).
    #[must_use]
    pub fn y_units(&self) -> Vec<&'static str> {
        self.y.iter().map(|c| c.unit()).collect()
    }
}
