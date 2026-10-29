// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGNozzle.{h,cpp}.
//
//! The nozzle (`FGNozzle`): the engine's vacuum thrust less the ambient
//! pressure on the exit area, never negative.

use super::families::{ThrusterFamily, ThrusterSite};
use super::thruster::ThrusterBase;
use crate::Error;
use crate::fdm::ModelEvaluator;
use crate::fdm::{LoadContext, Models};
use crate::math::E_X;
use crate::xml::Element;

/// `FGNozzle`'s members beyond `FGThruster`'s.
#[derive(Debug, Clone, PartialEq)]
pub struct Nozzle {
    /// Exit area, ft^2.
    pub area: f64,
}

impl ThrusterFamily for Nozzle {
    type Sample = ();

    /// `FGNozzle(exec, el, num)`: the exit area is mandatory.
    fn load(
        el: &Element,
        _thruster_el: &Element,
        base: &mut ThrusterBase,
        _ctx: &mut LoadContext<'_>,
        _site: &ThrusterSite<'_>,
    ) -> Result<Self, Error> {
        if el.find_element("area").is_none() {
            return Err(Error::Model {
                message: format!(
                    "{}: Fatal Error: Nozzle exit area must be given in nozzle config file.",
                    el.location()
                ),
            });
        }
        let area = el.find_element_value_as_number_convert_to("area", "FT2")?;
        base.thrust = 0.0;
        Ok(Self { area })
    }

    fn sample(
        &self,
        _base: &ThrusterBase,
        _models: &Models,
        _ev: &mut ModelEvaluator<'_>,
    ) -> Result<(), Error> {
        Ok(())
    }

    /// `FGNozzle::Calculate(vacThrust)`: the force is the thrust times the
    /// cosine of the reverser angle, which only a property of a direct
    /// thruster sets, but the thrust reported is not.
    fn calculate(&mut self, base: &mut ThrusterBase, vac_thrust: f64, _sample: &()) -> f64 {
        // `std::max(0.0, t)`: 0 unless t is above it.
        let t = vac_thrust - base.input.pressure * self.area;
        base.thrust = if 0.0 < t { t } else { 0.0 };
        base.force.native_force[E_X] = base.thrust * libm::cos(base.reverser_angle);
        base.thrust
    }
}

crate::snapshot::visit_fields! { Nozzle { area } }
