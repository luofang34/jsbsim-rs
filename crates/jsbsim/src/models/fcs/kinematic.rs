// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Anthony K. Peden.
// Derived from JSBSim src/models/flight_control/FGKinemat.{h,cpp}.
//
//! `<kinematic>`: the output moves towards the (scaled) input through a
//! list of detents, each interval traversed in its own time.

#[cfg(test)]
mod tests;

use super::component::{Base, Tick, model_error};
use super::families::{ComponentAt, Family};
use crate::Error;
use crate::fdm::LoadContext;
use crate::fdm::ModelEvaluator;
use crate::units::{constrain, equal_to_roundoff};
use crate::xml::Element;

/// `FGKinemat`.
#[derive(Debug, Clone)]
pub struct Kinemat {
    /// Detent positions, in `<setting>` order (at least two).
    pub detents: Vec<f64>,
    /// The time to reach each detent from the previous one; 0 or less is
    /// an infinite rate.
    pub transition_times: Vec<f64>,
    /// The input is scaled by the last detent unless `<noscale>` is given.
    pub do_scale: bool,
}

/// What one run reads: the input, then the first output node.
#[derive(Debug, Clone, Copy)]
pub struct Sample {
    input: f64,
    output: f64,
}

impl Kinemat {
    /// The index of the interval `[detents[ind - 1], detents[ind]]` the
    /// output is in, searched as JSBSim's `Run` does. JSBSim reads past the
    /// last detent when the output lies beyond it; the search stops at the
    /// last interval instead.
    fn interval(&self, input: f64, output: f64) -> usize {
        let last = self.detents.len().saturating_sub(1);
        let mut ind = 1;
        while ind < last {
            let Some(&detent) = self.detents.get(ind) else {
                break;
            };
            let inside = if input < output {
                detent < output
            } else {
                detent <= output
            };
            if !inside {
                break;
            }
            ind += 1;
        }
        ind
    }

    /// Moves `output` towards `input` for at most the step `dt`.
    fn traverse(&self, input: f64, mut output: f64, dt: f64) -> f64 {
        let mut dt0 = dt;
        while dt0 > 0.0 && !equal_to_roundoff(input, output) {
            let ind = self.interval(input, output);
            let (Some(&time), Some(&lo), Some(&hi)) = (
                self.transition_times.get(ind),
                self.detents.get(ind.wrapping_sub(1)),
                self.detents.get(ind),
            ) else {
                break;
            };
            if time <= 0.0 {
                output = input;
                break;
            }
            let rate = (hi - lo) / time;
            let this_input = constrain(lo, input, hi);
            let mut this_dt = ((this_input - output) / rate).abs();
            if dt0 < this_dt {
                this_dt = dt0;
                if output < input {
                    output += this_dt * rate;
                } else {
                    output -= this_dt * rate;
                }
            } else {
                // Separately, so the loop ends even in inexact arithmetic.
                output = this_input;
            }
            dt0 -= this_dt;
        }
        output
    }
}

impl Family for Kinemat {
    type Sample = Sample;

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        _at: ComponentAt,
    ) -> Result<Self, Error> {
        base.check_input_nodes(1, 1, el, ctx)?;
        let do_scale = el.find_element("noscale").is_none();
        // JSBSim dereferences the missing element.
        let traverse = el.find_element("traverse").ok_or_else(|| {
            model_error(
                el,
                format!("Kinematic component {} has no <traverse>", base.name),
            )
        })?;
        let mut detents = Vec::new();
        let mut transition_times = Vec::new();
        for setting in traverse.find_elements("setting") {
            detents.push(setting.find_element_value_as_number("position")?);
            transition_times.push(setting.find_element_value_as_number("time")?);
        }
        if detents.len() <= 1 {
            return Err(model_error(
                el,
                format!(
                    "\nKinematic component {} must have more than 1 setting element",
                    base.name
                ),
            ));
        }
        base.bind(ctx)?;
        Ok(Self {
            detents,
            transition_times,
            do_scale,
        })
    }

    fn sample(&self, base: &Base, ev: &mut ModelEvaluator<'_>) -> Result<Sample, Error> {
        let input = base.read_input(0, ev)?;
        // The first output (the component's own property without
        // <output>), so that an output set from outside moves the
        // component.
        let output = match base.outputs.first() {
            Some(&node) => ev.read_node(node)?,
            None => base.output,
        };
        Ok(Sample { input, output })
    }

    fn step(&mut self, base: &mut Base, s: Sample, tick: &Tick) {
        let (Some(&front), Some(&back)) = (self.detents.first(), self.detents.last()) else {
            return;
        };
        let mut input = s.input;
        if self.do_scale {
            input *= back;
        }
        base.input = constrain(front, input, back);
        base.output = if tick.trim_status {
            // While trimming the output is reached in one step.
            base.input
        } else {
            self.traverse(base.input, s.output, base.dt)
        };
        base.clip(tick);
    }
}

crate::snapshot::visit_fields! { Kinemat { detents, transition_times, do_scale } }
