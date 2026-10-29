// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2009 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGGyro.{h,cpp}.
//
//! `<gyro>`: the inertial body rate along a sensor axis, degraded as a
//! [`Signal`].

use super::component::{Base, Tick};
use super::families::{ComponentAt, Family};
use super::sensor::Signal;
use super::sensor::orientation::Orientation;
use crate::Error;
use crate::fdm::ModelEvaluator;
use crate::fdm::{LoadContext, Models};
use crate::xml::Element;

/// `FGGyro`.
#[derive(Debug, Clone)]
pub struct Gyro {
    /// The degradation.
    pub signal: Signal,
    /// Axis and frame.
    pub orientation: Orientation,
}

impl Family for Gyro {
    /// The measured rate (`Input`).
    type Sample = f64;

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        at: ComponentAt,
    ) -> Result<Self, Error> {
        let signal = Signal::load(el, base, ctx, at)?;
        Ok(Self {
            signal,
            orientation: Orientation::load(el, ctx)?,
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
        Ok(self.orientation.measure(models.propagate.state.pqri))
    }

    fn step(&mut self, base: &mut Base, input: f64, tick: &Tick) {
        base.input = input;
        self.signal.process(base, tick);
    }

    fn reset(&mut self, base: &mut Base) {
        self.signal.reset(base);
    }
}
