// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.h (GetSimTime, GetDeltaT) and
// src/models/FGPropagate.h (GetLocation, GetQuaternion, GetQuaternionECI,
// GetQuaternionECEF).
//
//! The clock and the vehicle's pose, read directly rather than through
//! properties. JSBSim binds the attitude only as Euler angles, which lose a
//! degree of freedom at ±90° pitch; the quaternions here do not.

use super::Fdm;
use crate::math::{Location, Quaternion};

#[cfg(test)]
mod tests;

impl Fdm {
    /// `GetSimTime`: the simulation time, s.
    #[must_use]
    pub fn sim_time(&self) -> f64 {
        self.models.exec.sim_time
    }

    /// `GetDeltaT`: the time step, s.
    #[must_use]
    pub fn dt(&self) -> f64 {
        self.models.exec.dt
    }

    /// Stands for the wall clock JSBSim reads with `time()`: seconds since
    /// the Unix epoch, UTC. A `<magnetometer>` dates its field model by it
    /// when it loads, so set it before loading; it is 0 (1 January 1970)
    /// until set. The core reads no clock of its own.
    pub fn set_wall_clock(&mut self, unix_seconds: i64) {
        self.models.exec.wall_clock = unix_seconds;
    }

    /// `GetLocation`: the vehicle's position in the ECEF frame.
    #[must_use]
    pub fn location(&self) -> &Location {
        &self.models.propagate.state.location
    }

    /// `GetQuaternion`: the attitude from the local NED frame to the body
    /// frame.
    #[must_use]
    pub fn quaternion(&self) -> &Quaternion {
        &self.models.propagate.state.q_attitude_local
    }

    /// `GetQuaternionECI`: the attitude from the ECI frame to the body
    /// frame.
    #[must_use]
    pub fn quaternion_eci(&self) -> &Quaternion {
        &self.models.propagate.state.q_attitude_eci
    }

    /// `GetQuaternionECEF`: the attitude from the ECEF frame to the body
    /// frame.
    #[must_use]
    pub fn quaternion_ecef(&self) -> &Quaternion {
        &self.models.propagate.q_ec2b
    }
}
