// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Copyright (C) 2012 T. Kreitler (t.kreitler@web.de)
// Derived from JSBSim src/models/propulsion/FGTransmission.{h,cpp}.
//
//! The rotor's transmission (`FGTransmission`): the engine and the rotor
//! as two inertias coupled through a free-wheel unit and a clutch, with
//! gear friction on the engine side and a rotor brake.

use super::families::{ThrusterKind, ThrusterSite};
use super::lag_filter::LagFilter;
use super::rotor::RpmDrive;
use crate::Error;
use crate::fdm::{Binder, Kind, Models};
use crate::units::constrain;

/// rpm per rad/s, as `FGTransmission.h` writes it.
const OMEGA_TO_RPM: f64 = 9.54929658551372;
/// rad/s per rpm, as `FGTransmission.h` writes it.
const RPM_TO_OMEGA: f64 = 0.10471975511965978;

/// `FGTransmission`.
#[derive(Debug, Clone, PartialEq)]
pub struct Transmission {
    /// Smooths the free-wheel unit's engagement.
    pub free_wheel_lag: LagFilter,
    /// 0 when the free-wheel unit has released, 1 when locked.
    pub free_wheel_transmission: f64,
    /// Moment of inertia of the rotor, slug·ft^2.
    pub thruster_moment: f64,
    /// Moment of inertia of the gear and engine, slug·ft^2.
    pub engine_moment: f64,
    /// Friction power of the gear and engine, ft·lbs/s.
    pub engine_friction: f64,
    /// Clutch, 0 (open) to 1 (engaged).
    pub clutch_ctrl_norm: f64,
    /// Rotor brake, 0 (off) to 1.
    pub brake_ctrl_norm: f64,
    /// Rotor brake power at full brake, ft·lbs/s.
    pub max_brake_power: f64,
    /// Engine side speed, rpm.
    pub engine_rpm: f64,
    /// Rotor side speed, rpm.
    pub thruster_rpm: f64,
}

impl Transmission {
    /// `FGTransmission(exec, num, dt)` up to `BindModel`.
    #[must_use]
    pub fn new(site: &ThrusterSite<'_>) -> Self {
        Self {
            free_wheel_lag: LagFilter::new(200.0, site.dt),
            free_wheel_transmission: 1.0,
            thruster_moment: 1.0,
            engine_moment: 1.0,
            engine_friction: 0.0,
            clutch_ctrl_norm: 1.0,
            brake_ctrl_norm: 0.0,
            max_brake_power: 0.0,
            engine_rpm: 0.0,
            thruster_rpm: 0.0,
        }
    }

    /// `InitDynamics`.
    pub fn init_dynamics(&mut self) {
        self.free_wheel_transmission = 1.0;
        self.engine_rpm = 0.0;
        self.thruster_rpm = 0.0;
    }

    /// `ResetToIC`.
    pub fn reset_to_ic(&mut self) {
        self.init_dynamics();
        self.free_wheel_lag.reset();
    }

    /// `Calculate(EnginePower, ThrusterTorque, dt)`: the engine and rotor
    /// speeds after `dt` of `engine_power` against `thruster_torque`.
    pub fn calculate(&mut self, engine_power: f64, mut thruster_torque: f64, dt: f64) {
        let engine_omega = self.engine_rpm * RPM_TO_OMEGA;
        let safe_engine_omega = if engine_omega < 1e-1 {
            1e-1
        } else {
            engine_omega
        };
        let mut engine_torque = engine_power / safe_engine_omega;
        let thruster_omega = self.thruster_rpm * RPM_TO_OMEGA;
        let safe_thruster_omega = if thruster_omega < 1e-1 {
            1e-1
        } else {
            thruster_omega
        };
        engine_torque -= self.engine_friction / safe_engine_omega;
        thruster_torque +=
            constrain(0.0, self.brake_ctrl_norm, 1.0) * self.max_brake_power / safe_thruster_omega;
        // Would the free-wheel unit release?
        let engine_d_omega = engine_torque / self.engine_moment * dt;
        let thruster_d_omega = -thruster_torque / self.thruster_moment * dt;
        self.free_wheel_transmission =
            if thruster_omega + thruster_d_omega > engine_omega + engine_d_omega {
                0.0
            } else {
                1.0
            };
        let fw_mult = self.free_wheel_lag.execute(self.free_wheel_transmission);
        let coupling = fw_mult * constrain(0.0, self.clutch_ctrl_norm, 1.0);
        let (jt, je) = (self.thruster_moment, self.engine_moment);
        if coupling < 0.999999 {
            // A linear transfer, then the transit to static friction.
            let engine_d_omega =
                (engine_torque - thruster_torque * coupling) / (jt * coupling + je) * dt;
            let thruster_d_omega =
                (engine_torque * coupling - thruster_torque) / (jt + je * coupling) * dt;
            self.engine_rpm += engine_d_omega * OMEGA_TO_RPM;
            self.thruster_rpm += thruster_d_omega * OMEGA_TO_RPM;
            let coupling_sq = coupling * coupling;
            self.engine_rpm = (1.0 - coupling_sq) * self.engine_rpm
                + coupling_sq * 0.02 * (49.0 * self.engine_rpm + self.thruster_rpm);
            self.thruster_rpm = (1.0 - coupling_sq) * self.thruster_rpm
                + coupling_sq * 0.02 * (self.engine_rpm + 49.0 * self.thruster_rpm);
            if (self.engine_rpm - self.thruster_rpm).abs() < 1e-3 {
                let mean = 0.5 * (self.engine_rpm + self.thruster_rpm);
                self.engine_rpm = mean;
                self.thruster_rpm = mean;
            }
        } else {
            let d_omega = (engine_torque - thruster_torque) / (jt + je) * dt;
            self.thruster_rpm += d_omega * OMEGA_TO_RPM;
            self.engine_rpm = self.thruster_rpm;
        }
        // Nothing turns backward.
        if self.engine_rpm < 0.0 {
            self.engine_rpm = 0.0;
        }
        if self.thruster_rpm < 0.0 {
            self.thruster_rpm = 0.0;
        }
    }
}

fn transmission(m: &Models, n: usize) -> Option<&Transmission> {
    match &m.propulsion.engines.get(n)?.thruster.kind {
        ThrusterKind::Rotor(r) => match &r.drive {
            RpmDrive::Transmission(t) => Some(t),
            RpmDrive::External(_) => None,
        },
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

fn transmission_mut(m: &mut Models, n: usize) -> Option<&mut Transmission> {
    match &mut m.propulsion.engines.get_mut(n)?.thruster.kind {
        ThrusterKind::Rotor(r) => match &mut r.drive {
            RpmDrive::Transmission(t) => Some(t),
            RpmDrive::External(_) => None,
        },
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

/// `FGTransmission::BindModel` for the rotor of engine `n`.
pub fn bind(b: &mut Binder<'_>, n: usize) -> Result<(), Error> {
    let base = format!("propulsion/engine[{n}]");
    b.property(
        &format!("{base}/brake-ctrl-norm"),
        Kind::Double,
        move |m| transmission(m, n).map_or(0.0, |t| t.brake_ctrl_norm),
        move |m, v| {
            if let Some(t) = transmission_mut(m, n) {
                t.brake_ctrl_norm = v;
            }
        },
    )?;
    b.property(
        &format!("{base}/clutch-ctrl-norm"),
        Kind::Double,
        move |m| transmission(m, n).map_or(0.0, |t| t.clutch_ctrl_norm),
        move |m, v| {
            if let Some(t) = transmission_mut(m, n) {
                t.clutch_ctrl_norm = v;
            }
        },
    )?;
    b.getter(&format!("{base}/free-wheel-transmission"), move |m| {
        transmission(m, n).map_or(0.0, |t| t.free_wheel_transmission)
    })?;
    Ok(())
}
