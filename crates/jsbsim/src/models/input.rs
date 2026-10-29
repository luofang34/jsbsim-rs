// SPDX-License-Identifier: LGPL-2.1-or-later
// FGInput: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// FGInputType, FGInputSocket, FGUDPInputSocket: Copyright (C) 2013
// Bertrand Coconnier.
// Derived from JSBSim src/models/FGInput.cpp (Run, Enable, Disable,
// Toggle), src/input_output/FGInputType.{h,cpp} and
// src/input_output/FGUDPInputSocket.cpp (Load's rate).
//
//! `FGInput` and its input types, without their sockets.
//!
//! Each `<input>` JSBSim loads becomes an input type with its own
//! declared properties and pre- and post-functions, which run every frame
//! the type is due unless a trim is in progress or the input is disabled.
//! The socket each type would read is never opened, so nothing else
//! writes the tree.

use super::base::ModelBase;
use crate::fdm::ModelFunctions;

/// `FGInput`.
#[derive(Debug, Clone, Default)]
pub struct Input {
    /// Scheduling, with `enabled` tied to
    /// `simulation/models/FGInput/enabled`.
    pub base: ModelBase,
    /// The input types, in load order.
    pub types: Vec<InputType>,
}

/// An `FGInputType`: its scheduling, declarations and functions.
#[derive(Debug, Clone, Default)]
pub struct InputType {
    /// `FGModel`'s scheduling. The rate is one frame unless a `QTJSBSIM`
    /// input with a port sets it from its `rate` attribute.
    pub base: ModelBase,
    /// The type's `<property>` declarations and pre- and post-functions.
    pub functions: ModelFunctions,
}

impl Input {
    /// `FGInput::Enable`: the model and every type.
    pub fn enable(&mut self) {
        self.base.enabled = true;
        for t in &mut self.types {
            t.base.enabled = true;
        }
    }

    /// `FGInput::Disable`: the model and every type.
    pub fn disable(&mut self) {
        self.base.enabled = false;
        for t in &mut self.types {
            t.base.enabled = false;
        }
    }

    /// `FGInput::Toggle(idx)`: flips type `idx`; its new state, or false
    /// when there is no such type.
    pub fn toggle(&mut self, idx: usize) -> bool {
        match self.types.get_mut(idx) {
            Some(t) => {
                t.base.enabled = !t.base.enabled;
                t.base.enabled
            }
            None => false,
        }
    }

    /// `FGInput::Run` before its types run: false when none runs this
    /// frame. Holding does not stop it.
    pub fn runs(&mut self, trim_status: bool) -> bool {
        !(trim_status || self.base.skip())
    }
}

/// `FGUDPInputSocket::Load`'s `SetRate(0.5 + 1.0/(dt*rate))` for an
/// integer `rate` attribute.
#[must_use]
pub fn udp_rate(rate: i32, dt: f64) -> u32 {
    // The double converts to `unsigned int` as `as` does on arm64:
    // truncating, and saturating out of range (a zero rate is infinite).
    (0.5 + 1.0 / (dt * f64::from(rate))) as u32
}
