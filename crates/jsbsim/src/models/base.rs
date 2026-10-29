// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGModel.{h,cpp} (rate and enable logic).
//
//! What every model shares: its execution rate and enable flag.

/// `FGModel`'s scheduling state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelBase {
    /// Runs once every `rate` frames.
    pub rate: u32,
    exe_ctr: u32,
    /// `simulation/models/<name>/enabled`.
    pub enabled: bool,
}

impl Default for ModelBase {
    fn default() -> Self {
        Self {
            rate: 1,
            exe_ctr: 1,
            enabled: true,
        }
    }
}

impl ModelBase {
    /// `FGModel::InitModel`.
    pub fn init(&mut self) {
        self.exe_ctr = 1;
    }

    /// `FGModel::Run`: true when the model must skip this frame.
    pub fn skip(&mut self) -> bool {
        if self.rate == 1 {
            return !self.enabled;
        }
        if self.exe_ctr >= self.rate {
            self.exe_ctr = 0;
        }
        let was = self.exe_ctr;
        self.exe_ctr = self.exe_ctr.wrapping_add(1);
        if was == 1 {
            return !self.enabled;
        }
        true
    }
}

crate::snapshot::visit_fields! { ModelBase { rate, exe_ctr, enabled } }
