// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.cpp (DoTrim).
//
//! The executive's trim entry point, which `simulation/do_simple_trim`
//! calls.

use super::Fdm;
use crate::Error;
use crate::trim::{Control, State, Trim, TrimMode, TrimReport};

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
        let trim = Trim::new(self, mode);
        self.run_trim(trim)
    }

    /// A [`TrimMode::Custom`] trim of `axes`, each a state and the control
    /// that trims it, in order: `FGTrim(fdm, tCustom)` with `AddState` for
    /// each. A state given twice keeps its first control, as `AddState`
    /// refuses the second. Otherwise as [`do_trim`](Self::do_trim).
    ///
    /// [`State::All`] is JSBSim's debug-axis selector (`tAll`), not a
    /// physical state to trim: its axis always counts as trimmed, leaving
    /// its control at the middle of the control's range.
    ///
    /// # Errors
    ///
    /// Those of [`do_trim`](Self::do_trim).
    pub fn do_trim_custom(&mut self, axes: &[(State, Control)]) -> Result<(), Error> {
        let mut trim = Trim::new(self, TrimMode::Custom);
        for &(state, control) in axes {
            trim.add_state(self, state, control);
        }
        self.run_trim(trim)
    }

    /// Runs `trim` and records its report, as `DoTrim` does.
    fn run_trim(&mut self, mut trim: Trim) -> Result<(), Error> {
        let success = trim.do_trim(self)?;
        let report = trim.report(self);
        if self.models.exec.debug_level > 0 {
            report.log();
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

    /// What the last trim the executive ran reported, as JSBSim's
    /// `FGTrim::Report` and `TrimStats` show it: one run by
    /// [`do_trim`](Self::do_trim), [`do_trim_custom`](Self::do_trim_custom)
    /// or `simulation/do_simple_trim`.
    #[must_use]
    pub fn last_trim(&self) -> Option<&TrimReport> {
        self.last_trim.as_ref()
    }

    /// How many trims the executive has run, wrapping: those of
    /// [`do_trim`](Self::do_trim), [`do_trim_custom`](Self::do_trim_custom)
    /// and `simulation/do_simple_trim`.
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
