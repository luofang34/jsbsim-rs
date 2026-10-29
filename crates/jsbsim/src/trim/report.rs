// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGTrim.cpp (Report, TrimStats) and
// FGTrimAxis.cpp (AxisReport).
//
//! What a trim reports: `FGTrim::Report` and `TrimStats` as data.

use super::{Control, State, Trim, TrimAxis, TrimMode};
use crate::fdm::Fdm;

/// One axis as `FGTrim::Report` and `TrimStats` show it.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisReport {
    /// The state.
    pub state: State,
    /// The control.
    pub control: Control,
    /// `GetControl`.
    pub control_value: f64,
    /// `GetState() + GetStateTarget()`, read when the report is made.
    pub state_value: f64,
    /// `GetTolerance`.
    pub tolerance: f64,
    /// `sub_iterations`: solver runs of the axis over all cycles.
    pub sub_iterations: f64,
    /// `successful`: cycles that ended with the axis in tolerance.
    pub successful: f64,
    /// `GetStability`: frames the axis' last run took to settle.
    pub stability: i32,
    /// `GetRunCount`: frames of all the axis' runs.
    pub run_count: i32,
    /// `GetAvgStability`.
    pub avg_stability: f64,
}

/// What `FGTrim::Report` and `TrimStats` show.
#[derive(Debug, Clone, PartialEq)]
pub struct TrimReport {
    /// The mode the trim ran in.
    pub mode: TrimMode,
    /// `DoTrim`'s result.
    pub success: bool,
    /// `total_its`: cycles run.
    pub total_iterations: u32,
    /// The axes, in trim order.
    pub axes: Vec<AxisReport>,
}

impl Trim {
    /// `Report` and `TrimStats`, with each state read now.
    pub fn report(&mut self, fdm: &Fdm) -> TrimReport {
        let axes = self
            .axes
            .iter_mut()
            .enumerate()
            .map(|(i, a)| AxisReport {
                state: a.state_type(),
                control: a.control_type(),
                control_value: a.control(),
                state_value: a.state(fdm) + a.state_target(),
                tolerance: a.tolerance(),
                sub_iterations: self.sub_iterations.get(i).copied().unwrap_or(0.0),
                successful: self.successful.get(i).copied().unwrap_or(0.0),
                stability: a.stability(),
                run_count: a.run_count(),
                avg_stability: a.avg_stability(),
            })
            .collect();
        TrimReport {
            mode: self.mode,
            success: self.success,
            total_iterations: self.total_its,
            axes,
        }
    }
}

/// `AxisReport`, logged.
pub(super) fn log_axis(axis: &mut TrimAxis, fdm: &Fdm) {
    let (control_convert, _) = axis.converts();
    let state = axis.state(fdm) + axis.state_target();
    let passed = state.abs() < axis.tolerance().abs();
    tracing::info!(
        "  {:<20}: {:<6.2} {:<5}: {:<9.2e} Tolerance: {:<3.0e}  {}",
        axis.control_type().name(),
        axis.control() * control_convert,
        axis.state_type().name(),
        state,
        axis.tolerance(),
        if passed { "Passed" } else { "Failed" }
    );
}
