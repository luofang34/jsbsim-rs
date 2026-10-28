// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000-2005 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGFCSFunction.{h,cpp}.
//
//! `<fcs_function>`: a function, scaled by the first input if any.

use super::component::{Base, Tick, model_error};
use super::families::{ComponentAt, Family};
use crate::Error;
use crate::fdm::LoadContext;
use crate::functions::{Evaluator, FnId};
use crate::xml::Element;

/// `FGFCSFunction`.
#[derive(Debug, Clone)]
pub struct FcsFunction {
    /// The function.
    pub function: FnId,
}

impl Family for FcsFunction {
    /// The function's value, then the first input when there is one.
    type Sample = (f64, Option<f64>);

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        _at: ComponentAt,
    ) -> Result<Self, Error> {
        let Some(function_el) = el.find_element("function") else {
            return Err(model_error(
                el,
                "FCS Function should contain a \"function\" element",
            ));
        };
        let function = ctx
            .functions
            .load_function(ctx.binder.tree, function_el, "")?;
        base.bind(ctx)?;
        Ok(Self { function })
    }

    fn sample(&self, base: &Base, ev: &mut Evaluator<'_>) -> Result<Self::Sample, Error> {
        let value = ev.function(self.function)?;
        let input = match base.inputs.is_empty() {
            true => None,
            false => Some(base.read_input(0, ev)?),
        };
        Ok((value, input))
    }

    fn step(&mut self, base: &mut Base, (value, input): Self::Sample, tick: &Tick) {
        base.output = value;
        if let Some(input) = input {
            base.input = input;
            base.output *= input;
        }
        base.clip(tick);
    }
}
