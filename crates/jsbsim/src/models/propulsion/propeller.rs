// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGPropeller.{h,cpp}.
//
//! The propeller (`FGPropeller`): fixed or variable pitch, the latter
//! either set externally or held at a commanded RPM by the
//! constant-speed governor, with feathering and reversing. Thrust and
//! power come from the `C_THRUST` and `C_POWER` tables over the advance
//! ratio (and blade angle), with optional helical-tip Mach and RPM
//! corrections; the engine power not absorbed accelerates the propeller.

mod bind;
mod load;
#[cfg(test)]
mod tests;

use std::f64::consts::PI;

use super::families::ThrusterFamily;
use super::thruster::ThrusterBase;
use crate::Error;
use crate::fdm::ModelEvaluator;
use crate::fdm::{LoadContext, Models};
use crate::functions::Table;
use crate::math::{ColumnVector3, E_U, E_V, E_W, E_X, E_Y, E_Z};
use crate::xml::Element;

/// The propeller's tables.
#[derive(Debug, Clone)]
pub struct PropellerTables {
    /// `C_THRUST`: thrust coefficient by advance ratio (and blade angle).
    pub c_thrust: Table,
    /// `C_POWER`: power coefficient by advance ratio (and blade angle).
    pub c_power: Table,
    /// `CT_MACH`: thrust factor by helical tip Mach.
    pub ct_mach: Option<Table>,
    /// `CP_MACH`: power factor by helical tip Mach.
    pub cp_mach: Option<Table>,
    /// `CT_RPM_FACTOR`: thrust factor by RPM.
    pub ct_rpm: Option<Table>,
    /// `CP_RPM_FACTOR`: power factor by RPM.
    pub cp_rpm: Option<Table>,
}

/// `FGPropeller`'s members beyond `FGThruster`'s.
#[derive(Debug, Clone)]
pub struct Propeller {
    /// Blades.
    pub num_blades: i32,
    /// Advance ratio.
    pub j: f64,
    /// Propeller speed, rpm.
    pub rpm: f64,
    /// Moment of inertia about the shaft, slug·ft^2.
    pub ixx: f64,
    /// Diameter, ft.
    pub diameter: f64,
    /// Blade angle range, deg; equal for a fixed-pitch propeller.
    pub max_pitch: f64,
    /// See `max_pitch`.
    pub min_pitch: f64,
    /// Governed speed range, rpm.
    pub min_rpm: f64,
    /// See `min_rpm`.
    pub max_rpm: f64,
    /// Blade angle, deg.
    pub pitch: f64,
    /// Shift of the thrust's acting location per radian of inflow angle.
    pub p_factor: f64,
    /// 1 clockwise, -1 counter-clockwise, seen from behind.
    pub sense: f64,
    /// -1 for a propeller file of version above 1, whose gyroscopic
    /// moment has the opposite sign.
    pub sense_multiplier: f64,
    /// Governor command, 0 (`min_rpm`) to 1 (`max_rpm`).
    pub advance: f64,
    /// Torque left to accelerate the propeller, lbs·ft.
    pub excess_torque: f64,
    /// Diameter to the fourth power.
    pub d4: f64,
    /// Diameter to the fifth power.
    pub d5: f64,
    /// Mach of the blade tips along their helix.
    pub helical_tip_mach: f64,
    /// Induced velocity at the disk, ft/s.
    pub vinduced: f64,
    /// Engine torque on the airframe, shaft axes, lbs·ft.
    pub torque: ColumnVector3,
    /// Coefficient tables.
    pub tables: Box<PropellerTables>,
    /// Thrust coefficient scale.
    pub ct_factor: f64,
    /// Power coefficient scale.
    pub cp_factor: f64,
    /// Nonzero for the constant-speed governor.
    pub constant_speed: i32,
    /// Blade angle when fully reversed, deg.
    pub reverse_pitch: f64,
    /// Reversed (turboprop beta range).
    pub reversed: bool,
    /// 0 (`min_pitch`) to 1 (`reverse_pitch`) while reversed.
    pub reverse_coef: f64,
    /// Feather command.
    pub feathered: bool,
}

impl Propeller {
    /// `InitDynamics`.
    pub fn init_dynamics(&mut self) {
        self.rpm = 0.0;
        self.pitch = self.min_pitch;
        self.vinduced = 0.0;
    }

    /// `IsVPitch`.
    #[must_use]
    pub fn is_vpitch(&self) -> bool {
        self.max_pitch != self.min_pitch
    }

    /// `GetPowerRequired`: the power the propeller absorbs at its speed,
    /// ft·lbs/s, after the governor (or the feathering or reversing) has
    /// moved the blades.
    pub fn power_required(&mut self, base: &mut ThrusterBase) -> f64 {
        let dt = base.input.total_delta_t;
        let mut c_preq = if self.is_vpitch() {
            if self.constant_speed != 0 {
                self.govern(dt);
            }
            self.tables.c_power.value_2d(self.j, self.pitch)
        } else {
            self.tables.c_power.value_1d(self.j)
        };
        c_preq *= self.cp_factor;
        if let Some(t) = &self.tables.cp_mach {
            c_preq *= t.value_1d(self.helical_tip_mach);
        }
        if let Some(t) = &self.tables.cp_rpm {
            c_preq *= t.value_1d(self.rpm);
        }
        let rps = self.rpm / 60.0;
        let local_rps = if rps < 0.01 { 0.01 } else { rps };
        base.power_required =
            c_preq * local_rps * local_rps * local_rps * self.d5 * base.input.density;
        base.power_required
    }

    /// The constant-speed mode of `GetPowerRequired`: the blade angle
    /// that holds the commanded RPM, or that feathers or reverses.
    fn govern(&mut self, dt: f64) {
        if self.feathered {
            // About 5 s to fully feathered.
            self.pitch += (self.max_pitch - self.pitch) / 300.0;
        } else if !self.reversed {
            let rpm_req = self.min_rpm + (self.max_rpm - self.min_rpm) * self.advance;
            let d_rpm = rpm_req - self.rpm;
            // Below 200 rpm the oil pump cannot move the blades.
            if self.rpm > 200.0 {
                self.pitch -= d_rpm * dt;
            }
            if self.pitch < self.min_pitch {
                self.pitch = self.min_pitch;
            } else if self.pitch > self.max_pitch {
                self.pitch = self.max_pitch;
            }
        } else {
            let pitch_req =
                self.min_pitch - (self.min_pitch - self.reverse_pitch) * self.reverse_coef;
            if self.rpm > 200.0 {
                self.pitch += (pitch_req - self.pitch) / 200.0;
            }
            if self.rpm > self.max_rpm {
                self.pitch += (self.max_rpm - self.rpm) / 50.0;
                if self.pitch < self.reverse_pitch {
                    self.pitch = self.reverse_pitch;
                } else if self.pitch > self.max_pitch {
                    self.pitch = self.max_pitch;
                }
            }
        }
    }

    /// The thrust coefficient at the current advance ratio, blade angle,
    /// tip Mach and RPM.
    fn thrust_coefficient(&self) -> f64 {
        let mut ct = if self.is_vpitch() {
            self.tables.c_thrust.value_2d(self.j, self.pitch)
        } else {
            self.tables.c_thrust.value_1d(self.j)
        };
        ct *= self.ct_factor;
        if let Some(t) = &self.tables.ct_mach {
            ct *= t.value_1d(self.helical_tip_mach);
        }
        if let Some(t) = &self.tables.ct_rpm {
            ct *= t.value_1d(self.rpm);
        }
        ct
    }

    /// The P-factor: the thrust's acting location shifted by a multiple
    /// of the inflow angle at the disk, which the induced velocity
    /// attenuates.
    fn p_factor_shift(&self, base: &mut ThrusterBase, local_aero_vel: ColumnVector3, vel: f64) {
        if self.p_factor <= 0.0001 {
            return;
        }
        let tangential_vel = local_aero_vel.magnitude2(E_V, E_W);
        if tangential_vel > 0.0001 {
            let angle = libm::atan2(tangential_vel, vel + self.vinduced);
            let factor = self.sense * self.p_factor * angle / tangential_vel;
            let force = &mut base.force;
            force.acting_location[E_Y] = force.location[E_Y] + factor * local_aero_vel[E_W];
            force.acting_location[E_Z] = force.location[E_Z] + factor * local_aero_vel[E_V];
        }
    }
}

impl ThrusterFamily for Propeller {
    /// The location, body frame, relative to the CG (`vDXYZ`), ft.
    type Sample = ColumnVector3;

    fn load(
        el: &Element,
        thruster_el: &Element,
        base: &mut ThrusterBase,
        ctx: &mut LoadContext<'_>,
    ) -> Result<Self, Error> {
        load::load(el, thruster_el, base, ctx)
    }

    fn sample(
        &self,
        base: &ThrusterBase,
        models: &Models,
        _ev: &mut ModelEvaluator<'_>,
    ) -> Result<ColumnVector3, Error> {
        Ok(models.mass_balance.structural_to_body(base.force.location))
    }

    /// `FGPropeller::Calculate(EnginePower)`: the thrust at the current
    /// speed in the local air flow, then the speed the excess torque
    /// gives over the step.
    fn calculate(
        &mut self,
        base: &mut ThrusterBase,
        engine_power: f64,
        v_dxyz: &ColumnVector3,
    ) -> f64 {
        // The transform of a custom force: its orientation's.
        let mt = base.force.transform;
        let input = &base.input;
        let (aero_uvw, aero_pqr, pqri) = (input.aero_uvw, input.aero_pqr, input.pqri);
        let (rho, soundspeed, dt) = (input.density, input.soundspeed, input.total_delta_t);
        // Stevens & Lewis eqn 8.2-1: the air velocity at the disk, wind
        // and turbulence included.
        let local_aero_vel = mt.transposed() * (aero_uvw + aero_pqr.cross(*v_dxyz));
        let vel = local_aero_vel[E_U];
        let rps = self.rpm / 60.0;
        let area = 0.25 * self.diameter * self.diameter * PI;
        let vtip = rps * self.diameter * PI;
        self.helical_tip_mach = (vtip * vtip + vel * vel).sqrt() / soundspeed;
        self.j = if rps > 0.01 {
            vel / (self.diameter * rps)
        } else {
            vel / self.diameter
        };
        let power_available = engine_power - self.power_required(base);
        base.thrust_coeff = self.thrust_coefficient();
        base.thrust = base.thrust_coeff * rps * rps * self.d4 * rho;
        // Momentum theory (McCormick eqn 6.15), with thrust and velocity
        // signed.
        let vel2sum = vel * vel.abs() + 2.0 * base.thrust / (rho * area);
        self.vinduced = if vel2sum > 0.0 {
            0.5 * (-vel + vel2sum.sqrt())
        } else {
            0.5 * (-vel - (-vel2sum).sqrt())
        };
        self.p_factor_shift(base, local_aero_vel, vel);
        let omega = rps * 2.0 * PI;
        base.force.native_force[E_X] = base.thrust;
        let omega_floor = if 0.01 < omega { omega } else { 0.01 };
        self.torque[E_X] = -self.sense * engine_power / omega_floor;
        // Gyroscopic momentum about the shaft; GetBodyForces transforms
        // it.
        let v_h = ColumnVector3::new(
            self.ixx * omega * self.sense * self.sense_multiplier,
            0.0,
            0.0,
        );
        self.excess_torque = if omega > 0.01 {
            power_available / omega
        } else {
            power_available / 1.0
        };
        self.rpm = (rps + ((self.excess_torque / self.ixx) / (2.0 * PI)) * dt) * 60.0;
        if self.rpm < 0.0 {
            self.rpm = 0.0;
        }
        base.force.native_moment = pqri.cross(mt * v_h) + mt * self.torque;
        base.thrust
    }

    /// `FGPropeller::ResetToIC`.
    fn reset_to_ic(&mut self, base: &mut ThrusterBase) {
        base.reset_to_ic();
        self.init_dynamics();
    }

    fn rpm(&self) -> f64 {
        self.rpm
    }

    fn engine_rpm(&self, base: &ThrusterBase) -> f64 {
        self.rpm * base.gear_ratio
    }

    fn set_rpm(&mut self, rpm: f64) {
        self.rpm = rpm;
    }

    fn set_engine_rpm(&mut self, base: &ThrusterBase, rpm: f64) {
        self.rpm = rpm / base.gear_ratio;
    }

    fn set_advance(&mut self, advance: f64) {
        self.advance = advance;
    }

    fn set_feather(&mut self, feather: bool) {
        self.feathered = feather;
    }

    fn is_propeller(&self) -> bool {
        true
    }

    fn set_reverse(&mut self, reversed: bool) {
        self.reversed = reversed;
    }

    fn set_reverse_coef(&mut self, coef: f64) {
        self.reverse_coef = coef;
    }

    fn torque(&self) -> f64 {
        self.torque[E_X]
    }
}
