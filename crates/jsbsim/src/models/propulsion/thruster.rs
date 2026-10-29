// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGThruster.{h,cpp} and
// FGEngine.cpp (LoadThrusterInputs).
//
//! What every thruster has (`FGThruster`'s members and its `FGForce`),
//! and the direct thruster, which is `FGThruster` itself. Thruster types
//! are registered in [`families`](super::families).

mod bind;
#[cfg(test)]
mod tests;

use super::Inputs;
use super::families::{ThrusterFamily, ThrusterKind};
use super::force::Force;
use crate::Error;
use crate::fdm::ModelEvaluator;
use crate::fdm::{Binder, LoadContext, Models};
use crate::math::{ColumnVector3, E_X, E_Y, E_Z, Matrix33};
use crate::xml::Element;

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
    /// The rest of `FGThruster`'s constructor on the thruster type's
    /// element `el` (a child of `<thruster>`): the location, its
    /// `propulsion/engine[n]/` ties, then either a fixed `<pointing>` or
    /// the `<orient>` angles with their ties (and, for `<direct>`, the
    /// reverser's), then `ResetToIC`.
    pub fn load(
        &mut self,
        thruster_el: &Element,
        el: &Element,
        binder: &mut Binder<'_>,
    ) -> Result<(), Error> {
        let n = self.engine_number;
        let location = match thruster_el.find_element("location") {
            Some(loc) => loc.find_element_triplet_convert_to("IN")?,
            None => {
                tracing::error!("{}: No thruster location found.", thruster_el.location());
                ColumnVector3::zero()
            }
        };
        self.force.set_location(location);
        bind::bind_location(binder, n)?;
        if let Some(pointing_el) = thruster_el.find_element("pointing") {
            // A fixed nozzle: no property gimbals or reverses it.
            let pointing = pointing_el.find_element_triplet_convert_to("RAD")?;
            let mut t = Matrix33::zero();
            t[(E_X, E_X)] = pointing[E_X];
            t[(E_Y, E_X)] = pointing[E_Y];
            t[(E_Z, E_X)] = pointing[E_Z];
            self.force.transform = t;
        } else {
            let orientation = match thruster_el.find_element("orient") {
                Some(orient) => orient.find_element_triplet_convert_to("RAD")?,
                None => ColumnVector3::zero(),
            };
            self.force.set_angles_to_body(orientation);
            bind::bind_orientation(binder, n)?;
            // Only a direct thruster can be reversed.
            if el.name() == "direct" {
                bind::bind_reverser(binder, n)?;
            }
        }
        self.reset_to_ic();
        Ok(())
    }

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

/// `<direct>`: `FGThruster` itself, whose thrust is the power it is
/// given, scaled by the reverser.
#[derive(Debug, Clone)]
pub struct Direct;

impl ThrusterFamily for Direct {
    type Sample = ();

    fn load(
        _el: &Element,
        _thruster_el: &Element,
        _base: &mut ThrusterBase,
        _ctx: &mut LoadContext<'_>,
    ) -> Result<Self, Error> {
        Ok(Self)
    }

    fn sample(
        &self,
        _base: &ThrusterBase,
        _models: &Models,
        _ev: &mut ModelEvaluator<'_>,
    ) -> Result<(), Error> {
        Ok(())
    }

    /// `FGThruster::Calculate(tt)`.
    fn calculate(&mut self, base: &mut ThrusterBase, power: f64, _sample: &()) -> f64 {
        base.thrust = libm::cos(base.reverser_angle) * power;
        base.force.native_force[E_X] = base.thrust;
        base.thrust
    }
}
