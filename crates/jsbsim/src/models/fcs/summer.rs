// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGSummer.{h,cpp}.
//
//! `<summer>`: the sum of every input plus a bias.

use super::component::{Base, Tick};
use super::families::{ComponentAt, Family};
use crate::Error;
use crate::fdm::LoadContext;
use crate::fdm::ModelEvaluator;
use crate::xml::Element;

/// `FGSummer`.
#[derive(Debug, Clone)]
pub struct Summer {
    /// `<bias>`, a number.
    pub bias: f64,
}

impl Family for Summer {
    /// The inputs summed in order, from 0.
    type Sample = f64;

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        _at: ComponentAt,
    ) -> Result<Self, Error> {
        let bias = match el.find_element("bias") {
            Some(_) => el.find_element_value_as_number("bias")?,
            None => 0.0,
        };
        base.bind(ctx)?;
        Ok(Self { bias })
    }

    fn sample(&self, base: &Base, ev: &mut ModelEvaluator<'_>) -> Result<f64, Error> {
        let mut sum = 0.0;
        for input in &base.inputs {
            sum += ev.property(input)?;
        }
        Ok(sum)
    }

    fn step(&mut self, base: &mut Base, sum: f64, tick: &Tick) {
        base.output = sum + self.bias;
        base.clip(tick);
    }
}
