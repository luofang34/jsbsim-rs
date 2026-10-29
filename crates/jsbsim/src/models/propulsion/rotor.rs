// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2010 T. Kreitler (t.kreitler@web.de)
// Derived from JSBSim src/models/propulsion/FGRotor.{h,cpp}.
//
//! The helicopter rotor (`FGRotor`), main, tail or tandem, after
//! Padfield's and Talbot's blade element theory (/SH79/): the inflow
//! lags towards the momentum-theory value, the thrust, coning and cyclic
//! flapping follow from the collective and cyclic controls and the hub's
//! motion, the hinge offset adds flapping moments, and ground effect
//! reduces the inflow close to the ground. Its speed is either the
//! [`Transmission`]'s, driven by the engine against the rotor torque, or
//! dictated externally: by a property or by another engine's rotor.

mod aero;
mod bind;
mod load;
#[cfg(test)]
mod tests;

use std::f64::consts::PI;

use super::families::{ThrusterFamily, ThrusterSite};
use super::lag_filter::LagFilter;
use super::thruster::ThrusterBase;
use super::transmission::Transmission;
use crate::Error;
use crate::fdm::{LoadContext, ModelEvaluator, Models};
use crate::math::{ColumnVector3, E_U, E_W, Matrix33};
use crate::properties::NodeId;
use crate::units::constrain;
use crate::xml::Element;

/// Thruster axes to hub-shaft axes (`TboToHsr`).
const TBO_TO_HSR: Matrix33 = Matrix33::new(0.0, 0.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0);
/// Hub-shaft axes to thruster axes (`HsrToTbo`), the transpose.
const HSR_TO_TBO: Matrix33 = Matrix33::new(0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0);

/// `FGRotor::eCtrlMapping`: which controls the rotor ties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMap {
    /// `eMainCtrl`: collective, lateral and longitudinal cyclic.
    Main,
    /// `eTailCtrl`: the collective as the anti-torque control.
    Tail,
    /// `eTandemCtrl`: the tail collective and both cyclics.
    Tandem,
}

/// The parameters `Configure` reads or estimates, and what it derives.
#[derive(Debug, Clone, PartialEq)]
pub struct RotorConfig {
    /// Step the inflow lag and the height filter were made for, s.
    pub dt: f64,
    /// ft.
    pub radius: f64,
    /// Blades.
    pub blade_num: i32,
    /// 1 counter-clockwise seen from above, -1 clockwise, 0 coaxial.
    pub sense: f64,
    /// Speed range, rpm.
    pub nominal_rpm: f64,
    /// See `nominal_rpm`.
    pub minimal_rpm: f64,
    /// See `nominal_rpm`.
    pub maximal_rpm: f64,
    /// ft.
    pub blade_chord: f64,
    /// 1/rad.
    pub lift_curve_slope: f64,
    /// Linear twist from root to tip, rad.
    pub blade_twist: f64,
    /// ft.
    pub hinge_offset: f64,
    /// One blade's moment of inertia about the flapping hinge, slug·ft^2.
    pub blade_flapping_moment: f64,
    /// One blade's mass times its CG's distance from the hinge, slug·ft.
    pub blade_mass_moment: f64,
    /// The rotor's moment of inertia about the shaft, slug·ft^2.
    pub polar_moment: f64,
    /// Time constant of the inflow, s.
    pub inflow_lag: f64,
    /// Tip loss factor.
    pub tip_loss_b: f64,
    /// Ground effect exponent; none below 1e-5.
    pub ground_effect_exp: f64,
    /// Height added to the filtered height above ground, ft.
    pub ground_effect_shift: f64,
    /// Lock number over the density.
    pub lock_number_by_rho: f64,
    /// Blade area over disk area.
    pub solidity: f64,
    /// Powers 0 to 4 of the radius.
    pub r: [f64; 5],
    /// Powers 0 to 4 of the tip loss factor.
    pub b: [f64; 5],
    /// Power of the rotor brake, ft·lbs/s.
    pub max_brake_power: f64,
    /// Gear friction power, ft·lbs/s (driven rotors only).
    pub gear_loss: f64,
    /// Moment of inertia behind the gear, slug·ft^2 (driven rotors only).
    pub gear_moment: f64,
}

/// What `InitDynamics` resets, with the density last read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RotorState {
    /// Density, slug/ft^3.
    pub rho: f64,
    /// Rotor speed, rpm.
    pub rpm: f64,
    /// Rotor speed, rad/s.
    pub omega: f64,
    /// Orientation of the control axes about the shaft, rad.
    pub beta_orient: f64,
    /// Coning angle, rad.
    pub a0: f64,
    /// Longitudinal flapping relative to the control axes, rad.
    pub a_1: f64,
    /// Lateral flapping relative to the control axes, rad.
    pub b_1: f64,
    /// Longitudinal flapping of the downwash, for the drag, rad.
    pub a_dw: f64,
    /// Longitudinal flapping relative to the shaft, rad.
    pub a1s: f64,
    /// Lateral flapping relative to the shaft, rad.
    pub b1s: f64,
    /// Drag force, lbs.
    pub h_drag: f64,
    /// Side force, lbs.
    pub j_side: f64,
    /// Torque, lbs·ft.
    pub torque: f64,
    /// Thrust coefficient.
    pub c_t: f64,
    /// Inflow ratio.
    pub lambda: f64,
    /// Advance (tip-speed) ratio.
    pub mu: f64,
    /// Induced inflow ratio.
    pub nu: f64,
    /// Induced velocity, ft/s.
    pub v_induced: f64,
    /// Downwash angle towards the nose, rad.
    pub theta_downwash: f64,
    /// Downwash angle to the left, rad.
    pub phi_downwash: f64,
    /// Engine speed through the gear, rpm.
    pub engine_rpm: f64,
}

impl RotorState {
    /// `InitDynamics`, which leaves the density.
    pub fn init_dynamics(&mut self) {
        *self = Self {
            rho: self.rho,
            lambda: -0.001,
            nu: 0.001,
            ..Self::default()
        };
    }
}

/// The controls the rotor's properties set.
#[derive(Debug, Clone, PartialEq)]
pub struct RotorControls {
    /// Which controls are tied.
    pub map: ControlMap,
    /// Collective pitch (anti-torque for a tail rotor), rad.
    pub collective: f64,
    /// Lateral cyclic, rad.
    pub lateral: f64,
    /// Longitudinal cyclic, rad.
    pub longitudinal: f64,
    /// Scales the ground effect.
    pub ground_effect_scale_norm: f64,
}

/// A speed dictated from outside (`ExternalRPM`).
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalRpm {
    /// The engine whose rotor sets the speed, or -1 for the
    /// `x-rpm-dict` property.
    pub definition: i32,
    /// The property read for the speed, if found.
    pub source: Option<NodeId>,
    /// Gear ratio of the source engine's thruster.
    pub source_gear_ratio: f64,
}

/// What sets the rotor's speed.
#[derive(Debug, Clone, PartialEq)]
pub enum RpmDrive {
    /// The engine, through the transmission.
    Transmission(Transmission),
    /// A property.
    External(ExternalRpm),
}

/// `FGRotor`'s members beyond `FGThruster`'s.
#[derive(Debug, Clone)]
pub struct Rotor {
    /// Parameters.
    pub config: RotorConfig,
    /// Dynamic state.
    pub state: RotorState,
    /// Controls.
    pub controls: RotorControls,
    /// What sets the speed.
    pub drive: RpmDrive,
    /// Body to thruster axes, updated every calculation.
    pub inv_transform: Matrix33,
    /// Smooths the height above ground for the ground effect at 1 rad/s.
    pub damp_hagl: LagFilter,
}

/// What a rotor's calculation reads: where its hub is and the external
/// speed.
#[derive(Debug)]
pub struct RotorSample {
    /// The acting location relative to the CG, body frame, ft.
    pub hub: ColumnVector3,
    /// The value of the external speed's property.
    pub external_rpm: Option<f64>,
}

impl Rotor {
    /// `CalcRotorState`: the forces and moments at the current speed and
    /// controls into the force's native force and moment.
    fn calc_rotor_state(&mut self, base: &mut ThrusterBase, sample: &RotorSample) {
        let input = &base.input;
        self.state.rho = input.density;
        let mut h_agl_ft = input.h_agl;
        let (uvw, pqr) = (input.aero_uvw, input.aero_pqr);
        let transform = base.force.transform;
        self.inv_transform = transform.transposed();
        if let (RpmDrive::External(ext), Some(source)) = (&self.drive, sample.external_rpm) {
            self.state.rpm = source * (ext.source_gear_ratio / base.gear_ratio);
        }
        let c = &self.config;
        self.state.rpm = constrain(c.minimal_rpm, self.state.rpm, c.maximal_rpm);
        self.state.omega = (self.state.rpm / 60.0) * 2.0 * PI;
        let a_ic = self.controls.lateral;
        let b_ic = self.controls.longitudinal;
        let theta_col = self.controls.collective;
        // A factor of 1 is no effect, 0.5 the most.
        let mut ge_factor = 1.0;
        if c.ground_effect_exp > 1e-5 {
            if h_agl_ft < 0.0 {
                h_agl_ft = 0.0;
            }
            let filtered_hagl = self.damp_hagl.execute(h_agl_ft) + c.ground_effect_shift;
            // The actual over nominal speed keeps the scale sane at startup.
            ge_factor -= self.controls.ground_effect_scale_norm
                * (libm::exp(-filtered_hagl * c.ground_effect_exp)
                    * (self.state.rpm / c.nominal_rpm));
            ge_factor = constrain(0.5, ge_factor, 1.0);
        }
        let v_hub = self.hub_vel_body2ca(uvw, pqr, sample.hub, a_ic, b_ic);
        let av_fus = self.fus_angvel_body2ca(pqr);
        let thrust = self.calc_flow_and_thrust(theta_col, v_hub[E_U], v_hub[E_W], ge_factor);
        base.thrust = thrust;
        self.calc_coning_angle(theta_col);
        self.calc_flapping_angles(theta_col, av_fus);
        self.calc_drag_and_side_forces(theta_col, thrust);
        self.calc_torque(thrust);
        self.calc_downwash_angles(uvw);
        base.force.native_force = self.body_forces(a_ic, b_ic, thrust);
        base.force.native_moment = transform * self.body_moments(a_ic, b_ic);
    }
}

impl ThrusterFamily for Rotor {
    type Sample = RotorSample;

    fn load(
        el: &Element,
        thruster_el: &Element,
        base: &mut ThrusterBase,
        ctx: &mut LoadContext<'_>,
        site: &ThrusterSite<'_>,
    ) -> Result<Self, Error> {
        Self::load_rotor(el, thruster_el, base, ctx, site)
    }

    fn sample(
        &self,
        base: &ThrusterBase,
        models: &Models,
        ev: &mut ModelEvaluator<'_>,
    ) -> Result<RotorSample, Error> {
        let external_rpm = match &self.drive {
            RpmDrive::External(ExternalRpm {
                source: Some(node), ..
            }) => Some(ev.read_node(*node)?),
            _ => None,
        };
        Ok(RotorSample {
            hub: models
                .mass_balance
                .structural_to_body(base.force.acting_location),
            external_rpm,
        })
    }

    /// `FGRotor::Calculate(EnginePower)`: the rotor state at its speed,
    /// then the speed after the step, the transmission's or the external
    /// one, within the speed range.
    fn calculate(&mut self, base: &mut ThrusterBase, power: f64, sample: &RotorSample) -> f64 {
        self.calc_rotor_state(base, sample);
        match &mut self.drive {
            RpmDrive::Transmission(t) => {
                t.calculate(power, self.state.torque, base.input.total_delta_t);
                self.state.engine_rpm = t.engine_rpm * base.gear_ratio;
                self.state.rpm = t.thruster_rpm;
            }
            RpmDrive::External(_) => {
                self.state.engine_rpm = self.state.rpm * base.gear_ratio;
            }
        }
        let c = &self.config;
        self.state.rpm = constrain(c.minimal_rpm, self.state.rpm, c.maximal_rpm);
        base.thrust
    }

    /// `FGRotor::ResetToIC`: the thruster's, the dynamics, the height
    /// filter and the transmission.
    fn reset_to_ic(&mut self, base: &mut ThrusterBase) {
        base.reset_to_ic();
        self.state.init_dynamics();
        self.damp_hagl.reset();
        if let RpmDrive::Transmission(t) = &mut self.drive {
            t.reset_to_ic();
        }
    }

    fn rpm(&self) -> f64 {
        self.state.rpm
    }

    fn engine_rpm(&self, _base: &ThrusterBase) -> f64 {
        self.state.engine_rpm
    }

    fn set_rpm(&mut self, rpm: f64) {
        self.state.rpm = rpm;
    }

    fn set_engine_rpm(&mut self, _base: &ThrusterBase, rpm: f64) {
        self.state.engine_rpm = rpm;
    }
}
