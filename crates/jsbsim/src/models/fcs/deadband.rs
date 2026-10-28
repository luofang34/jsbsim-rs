// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGDeadBand.{h,cpp}.
//
//! `<deadband>`: zero within half the width either side of 0, the input
//! moved towards 0 by that half width outside.

use super::component::{Base, Tick, parameter_value};
use super::families::{ComponentAt, Family};
use crate::Error;
use crate::fdm::LoadContext;
use crate::functions::{Evaluator, Parameter};
use crate::xml::Element;

/// `FGDeadBand`.
#[derive(Debug, Clone)]
pub struct DeadBand {
    /// `<width>`, 0 by default.
    pub width: Parameter,
    /// `<gain>`, a number, 1 by default.
    pub gain: f64,
}

impl Family for DeadBand {
    /// The input, then the width.
    type Sample = (f64, f64);

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        _at: ComponentAt,
    ) -> Result<Self, Error> {
        base.check_input_nodes(1, 1, el, ctx)?;
        let width = match el.find_element("width") {
            Some(width_el) => parameter_value(width_el, ctx)?,
            None => Parameter::Real(0.0),
        };
        let gain = match el.find_element("gain") {
            Some(_) => el.find_element_value_as_number("gain")?,
            None => 1.0,
        };
        base.bind(ctx)?;
        Ok(Self { width, gain })
    }

    fn sample(&self, base: &Base, ev: &mut Evaluator<'_>) -> Result<Self::Sample, Error> {
        let input = base.read_input(0, ev)?;
        Ok((input, ev.parameter(&self.width)?))
    }

    fn step(&mut self, base: &mut Base, (input, width): Self::Sample, tick: &Tick) {
        base.input = input;
        // A negative width makes both comparisons below pass the other way
        // round; JSBSim does not guard against it.
        let half_width = 0.5 * width;
        base.output = if input < -half_width {
            (input + half_width) * self.gain
        } else if input > half_width {
            (input - half_width) * self.gain
        } else {
            0.0
        };
        base.clip(tick);
    }
}
