// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGTrim.h.
//
//! The trim's state. It lives in a crate-private module so that only
//! [`Fdm::do_trim`](crate::Fdm::do_trim) drives it outside the `__oracle`
//! feature; its methods are in the parent module.

use super::{Solver, State, TrimAxis, TrimMode};
use crate::initial_condition::InitialCondition;

/// `FGTrim`.
#[derive(Debug, Clone)]
pub struct Trim {
    pub(super) axes: Vec<TrimAxis>,
    pub(super) mode: TrimMode,
    pub(super) fgic: InitialCondition,
    pub(super) solver: Solver,
    pub(super) debug_level: i32,
    pub(super) sub_iterations: Vec<f64>,
    pub(super) successful: Vec<f64>,
    pub(super) solution: Vec<bool>,
    pub(super) max_iterations: u32,
    pub(super) total_its: u32,
    pub(super) gamma_fallback: bool,
    pub(super) target_nlf: f64,
    pub(super) debug_axis: State,
    pub(super) psidot: f64,
    pub(super) success: bool,
}
