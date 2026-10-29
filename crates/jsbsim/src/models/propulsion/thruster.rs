// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGThruster.h and
// FGEngine.cpp (LoadThrusterInputs).
//
//! What every thruster has (`FGThruster`'s members and its `FGForce`).
//! Thruster types are registered in [`families`](super::families).

use super::Inputs;
use super::families::ThrusterKind;
use super::force::Force;
use crate::math::ColumnVector3;

/// `FGThruster::Inputs`, which the engine copies from its own before it
/// calculates the thrust (`LoadThrusterInputs`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThrusterInputs {
    /// Time step times the propulsion rate, s.
    pub total_delta_t: f64,
    /// Height above ground, ft.
    pub h_agl: f64,
    /// Inertial body rates, rad/s.
    pub pqri: ColumnVector3,
    /// Aero-relative body rates, rad/s.
    pub aero_pqr: ColumnVector3,
    /// Aero-relative body velocity, ft/s.
    pub aero_uvw: ColumnVector3,
    /// Density, slug/ft^3.
    pub density: f64,
    /// Static pressure, psf.
    pub pressure: f64,
    /// Speed of sound, ft/s.
    pub soundspeed: f64,
    /// Angle of attack, rad.
    pub alpha: f64,
    /// Sideslip, rad.
    pub beta: f64,
    /// True airspeed, ft/s.
    pub vt: f64,
}

impl ThrusterInputs {
    /// `FGEngine::LoadThrusterInputs`.
    pub fn load(&mut self, input: &Inputs) {
        self.total_delta_t = input.total_delta_t;
        self.h_agl = input.h_agl;
        self.pqri = input.pqri;
        self.aero_pqr = input.aero_pqr;
        self.aero_uvw = input.aero_uvw;
        self.density = input.density;
        self.pressure = input.pressure;
        self.soundspeed = input.soundspeed;
        self.alpha = input.alpha;
        self.beta = input.beta;
        self.vt = input.vt;
    }
}

/// `FGThruster`'s members, and the `FGForce` it is.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThrusterBase {
    /// The force, its location and orientation.
    pub force: Force,
    /// `name` attribute of the thruster type's element.
    pub name: String,
    /// The engine it belongs to.
    pub engine_number: usize,
    /// Thrust of the last calculation, lbs.
    pub thrust: f64,
    /// Power required, ft·lbs/s.
    pub power_required: f64,
    /// Engine to thruster gear ratio.
    pub gear_ratio: f64,
    /// Thrust coefficient.
    pub thrust_coeff: f64,
    /// Reverser angle, rad: the thrust is scaled by its cosine.
    pub reverser_angle: f64,
    /// Inputs.
    pub input: ThrusterInputs,
}

impl ThrusterBase {
    /// `FGThruster::ResetToIC`.
    pub fn reset_to_ic(&mut self) {
        self.reverser_angle = 0.0;
        self.thrust = 0.0;
        self.force.acting_location = self.force.location;
    }
}

/// A thruster: what every thruster has, and its type's own state.
#[derive(Debug, Clone)]
pub struct Thruster {
    /// What every thruster has.
    pub base: ThrusterBase,
    /// The thruster type and its state.
    pub kind: ThrusterKind,
}
