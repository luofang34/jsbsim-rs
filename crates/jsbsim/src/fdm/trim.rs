// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.cpp (DoTrim).
//
//! The executive's trim entry point, which `simulation/do_simple_trim`
//! calls.

use super::Fdm;
use crate::Error;
use crate::trim::{Trim, TrimMode, TrimReport};

impl Fdm {
    /// `DoTrim(mode)`: trims in `mode` and sets `simulation/trim-completed`.
    /// A trim that does not converge is [`Error::TrimFailed`], as JSBSim
    /// throws `TrimFailureException`; the failed trim has put the controls
    /// back.
    ///
    /// # Errors
    ///
    /// [`Error::TrimFailed`] when the trim does not converge, and
    /// [`Error::Function`] when a function the trimmed frames evaluate
    /// fails.
    pub fn do_trim(&mut self, mode: TrimMode) -> Result<(), Error> {
        let mut trim = Trim::new(self, mode);
        let success = trim.do_trim(self)?;
        let report = trim.report(self);
        if self.models.exec.debug_level > 0 {
            log_report(&report);
        }
        self.last_trim = Some(report);
        self.trims_run = self.trims_run.wrapping_add(1);
        if !success {
            return Err(Error::TrimFailed {
                message: "Trim Failed".to_owned(),
            });
        }
        self.models.exec.trim_completed = 1;
        Ok(())
    }

    /// `DoTrim(mode)` with the `int` that `simulation/do_simple_trim`
    /// receives: a value outside JSBSim's `tModes` is
    /// [`Error::TrimFailed`] with JSBSim's "Illegal trimming mode!".
    pub(super) fn do_trim_requested(&mut self, mode: i32) -> Result<(), Error> {
        let Some(mode) = TrimMode::from_i32(mode) else {
            return Err(Error::TrimFailed {
                message: "Illegal trimming mode!".to_owned(),
            });
        };
        self.do_trim(mode)
    }

    /// What the last [`do_trim`](Self::do_trim) reported, as JSBSim's
    /// `FGTrim::Report` and `TrimStats` show it.
    #[must_use]
    pub fn last_trim(&self) -> Option<&TrimReport> {
        self.last_trim.as_ref()
    }

    /// How many trims [`do_trim`](Self::do_trim) has run, wrapping.
    #[must_use]
    pub fn trims_run(&self) -> u32 {
        self.trims_run
    }

    /// The trim mode the initial conditions request (`TrimRequested`).
    #[must_use]
    pub fn trim_requested(&self) -> TrimMode {
        self.models.ic.trim_requested()
    }
}

/// `FGTrim::Report`, logged.
fn log_report(report: &TrimReport) {
    tracing::info!("  Trim Results:");
    for a in &report.axes {
        tracing::info!(
            "  {:<20}: {:<6.2} {:<5}: {:<9.2e} Tolerance: {:<3.0e}  {}",
            a.control.name(),
            a.control_value,
            a.state.name(),
            a.state_value,
            a.tolerance,
            if a.state_value.abs() < a.tolerance.abs() {
                "Passed"
            } else {
                "Failed"
            }
        );
    }
}
