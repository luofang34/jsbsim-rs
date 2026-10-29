// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2005 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGAccelerometer.{h,cpp}.
//
//! `<accelerometer>`: the acceleration at a structural location, along a
//! sensor axis, degraded as a [`Signal`].

use super::component::{Base, Tick, model_error};
use super::families::{ComponentAt, Family};
use super::sensor::Signal;
use super::sensor::orientation::Orientation;
use crate::Error;
use crate::fdm::ModelEvaluator;
use crate::fdm::{LoadContext, Models};
use crate::math::ColumnVector3;
use crate::xml::Element;

/// `FGAccelerometer`.
#[derive(Debug, Clone)]
pub struct Accelerometer {
    /// The degradation.
    pub signal: Signal,
    /// Axis and frame.
    pub orientation: Orientation,
    /// `<location>`, structural frame, in.
    pub location: ColumnVector3,
}

impl Family for Accelerometer {
    /// The measured acceleration (`Input`).
    type Sample = f64;

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        at: ComponentAt,
    ) -> Result<Self, Error> {
        let signal = Signal::load(el, base, ctx, at)?;
        let orientation = Orientation::load(el, ctx)?;
        let Some(location_el) = el.find_element("location") else {
            return Err(model_error(el, "No location given for accelerometer."));
        };
        Ok(Self {
            signal,
            orientation,
            location: location_el.find_element_triplet_convert_to("IN")?,
        })
    }

    fn sample(&self, base: &Base, _ev: &mut ModelEvaluator<'_>) -> Result<f64, Error> {
        Err(super::sensor::needs_models(base))
    }

    fn sample_models(
        &self,
        _base: &Base,
        models: &Models,
        _ev: &mut ModelEvaluator<'_>,
    ) -> Result<f64, Error> {
        let radius = models.mass_balance.structural_to_body(self.location);
        let a = &models.accelerations;
        let pqri = models.propagate.state.pqri;
        let accel = a.body_accel + a.pqridot.cross(radius) + pqri.cross(pqri.cross(radius));
        Ok(self.orientation.measure(accel))
    }

    fn step(&mut self, base: &mut Base, input: f64, tick: &Tick) {
        base.input = input;
        self.signal.process(base, tick);
    }

    fn reset(&mut self, base: &mut Base) {
        self.signal.reset(base);
    }
}
