// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000-2005 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGFCSComponent.{h,cpp}.
//
//! What every FCS component shares (`FGFCSComponent`): its inputs and
//! outputs, clipping, the transport delay and its own `fcs/<name>`
//! property.

use crate::Error;
use crate::fdm::LoadContext;
use crate::functions::{Evaluator, Parameter, PropertyValue};
use crate::properties::{NodeId, mk_property_name};
use crate::units::constrain;
use crate::xml::Element;

/// `<clipto>` limits.
#[derive(Debug, Clone)]
pub struct Clip {
    /// Lower bound.
    pub min: Parameter,
    /// Upper bound.
    pub max: Parameter,
    /// Wrap into the range instead of saturating.
    pub cyclic: bool,
}

/// The clip limits read for one run: `(min, max)`.
pub type Limits = (f64, f64);

/// What a family's `step` may read besides its own state and sample.
#[derive(Debug, Clone, Copy)]
pub struct Tick {
    /// The clip limits read this run, when the component clips.
    pub clip: Option<Limits>,
    /// `GetTrimStatus`.
    pub trim_status: bool,
}

/// `FGFCSComponent`'s members.
#[derive(Debug, Clone)]
pub struct Base {
    /// `name` attribute.
    pub name: String,
    /// `<input>` operands, in document order.
    pub inputs: Vec<PropertyValue>,
    /// `<output>` nodes, then the component's own property once bound.
    pub outputs: Vec<NodeId>,
    /// Clipping, when both limits are given.
    pub clip: Option<Clip>,
    /// The transport delay buffer (`output_array`); its length is the
    /// delay in frames.
    pub delay: Vec<f64>,
    /// Next slot of `delay`.
    pub index: usize,
    /// `GetChannelDeltaT` when the component was loaded.
    pub dt: f64,
    /// Last input read.
    pub input: f64,
    /// Last output.
    pub output: f64,
}

/// Logs `message` at `el`, or fails with it in strict mode.
pub fn lenient(el: &Element, ctx: &LoadContext<'_>, message: &str) -> Result<(), Error> {
    if ctx.options.strict {
        return Err(model_error(el, message));
    }
    tracing::error!("{}: {message}", el.location());
    Ok(())
}

/// A load error at `el`.
pub fn model_error(el: &Element, message: impl std::fmt::Display) -> Error {
    Error::Model {
        message: format!("{}: {message}", el.location()),
    }
}

/// `FGParameterValue(el, pm)`: exactly one non-empty data line, a number
/// or a property.
pub fn parameter_value(el: &Element, ctx: &mut LoadContext<'_>) -> Result<Parameter, Error> {
    let value = el.data_line(0);
    let parameter = Parameter::number_or_property(value, ctx.tree(), el.location())?;
    if el.data_lines().len() != 1 || value.is_empty() {
        return Err(model_error(
            el,
            format!(
                "The element <{}> must either contain a value number or a property name.",
                el.name()
            ),
        ));
    }
    Ok(parameter)
}

/// An output node; a node created here starts at the component's output,
/// 0.
fn output_node(path: &str, ctx: &mut LoadContext<'_>) -> Result<NodeId, Error> {
    let tree = ctx.tree();
    let existed = tree.has_node(path)?;
    let node = tree.get_or_create(tree.root(), path)?;
    if !existed {
        tree.set_double(node, 0.0);
    }
    Ok(node)
}

/// The component's own property path (`bind`): the name itself when it
/// holds a `/`, else `fcs/` and the name made a property name.
#[must_use]
pub fn own_property(name: &str) -> String {
    if name.contains('/') {
        name.to_owned()
    } else {
        format!("fcs/{}", mk_property_name(name, true))
    }
}

impl Base {
    /// `FGFCSComponent(fcs, el)`, with `dt` the channel's step.
    pub fn load(el: &Element, ctx: &mut LoadContext<'_>, dt: f64) -> Result<Self, Error> {
        for init in el.find_elements("init") {
            // Parsed and kept by JSBSim, but no component reads them.
            PropertyValue::new(init.data_line(0), ctx.tree(), init.location())?;
        }
        let mut inputs = Vec::new();
        for input in el.find_elements("input") {
            inputs.push(PropertyValue::new(
                input.data_line(0),
                ctx.tree(),
                input.location(),
            )?);
        }
        let mut outputs = Vec::new();
        for out in el.find_elements("output") {
            outputs.push(output_node(out.data_line(0), ctx)?);
        }
        let delay = match el.find_element("delay") {
            Some(delay_el) => vec![0.0; delay_frames(delay_el, ctx, dt)?],
            None => Vec::new(),
        };
        Ok(Self {
            name: el.attribute("name").to_owned(),
            inputs,
            outputs,
            clip: load_clip(el, ctx)?,
            delay,
            index: 0,
            dt,
            input: 0.0,
            output: 0.0,
        })
    }

    /// `bind`: the component's own property joins the outputs.
    pub fn bind(&mut self, ctx: &mut LoadContext<'_>) -> Result<(), Error> {
        let node = output_node(&own_property(&self.name), ctx)?;
        self.outputs.push(node);
        Ok(())
    }

    /// `CheckInputNodes(min, max, el)`.
    pub fn check_input_nodes(
        &self,
        min: usize,
        max: usize,
        el: &Element,
        ctx: &LoadContext<'_>,
    ) -> Result<(), Error> {
        let num = self.inputs.len();
        if num < min {
            return Err(model_error(
                el,
                format!(
                    "    Not enough <input> nodes are provided\n    Expecting {min} while {num} are provided."
                ),
            ));
        }
        if num > max {
            lenient(
                el,
                ctx,
                &format!(
                    "    Too many <input> nodes are provided\n    Expecting {max} while {num} are provided.\n    The last {} input nodes will be ignored.",
                    num - max
                ),
            )?;
        }
        Ok(())
    }

    /// `InputNodes[i]->getDoubleValue()`.
    pub fn read_input(&self, i: usize, ev: &mut Evaluator<'_>) -> Result<f64, Error> {
        match self.inputs.get(i) {
            Some(input) => ev.property(input),
            None => Ok(0.0),
        }
    }

    /// The clip limits, read as `Clip` reads them.
    pub fn clip_limits(&self, ev: &mut Evaluator<'_>) -> Result<Option<Limits>, Error> {
        match &self.clip {
            Some(clip) => Ok(Some((ev.parameter(&clip.min)?, ev.parameter(&clip.max)?))),
            None => Ok(None),
        }
    }

    /// `Clip`.
    pub fn clip(&mut self, tick: &Tick) {
        let (Some(clip), Some((vmin, vmax))) = (&self.clip, tick.clip) else {
            return;
        };
        let range = vmax - vmin;
        if range < 0.0 {
            tracing::error!(
                "Trying to clip {} with a max value ({vmax}) lower than the min value ({vmin}). Clipping is ignored.",
                self.name
            );
            return;
        }
        if clip.cyclic && range != 0.0 {
            let value = self.output - vmin;
            self.output = libm::fmod(value, range) + vmin;
            if self.output < vmin {
                self.output += range;
            }
        } else {
            self.output = constrain(vmin, self.output, vmax);
        }
    }

    /// `Delay`: while trimming the whole history takes the output.
    pub fn delay(&mut self, tick: &Tick) {
        if tick.trim_status {
            self.delay.fill(self.output);
            return;
        }
        let len = self.delay.len();
        if let Some(slot) = self.delay.get_mut(self.index) {
            *slot = self.output;
        }
        self.index = if self.index + 1 >= len {
            0
        } else {
            self.index + 1
        };
        if let Some(&delayed) = self.delay.get(self.index) {
            self.output = delayed;
        }
    }

    /// `FGFCSComponent::ResetPastStates`.
    pub fn reset_past_states(&mut self) {
        self.index = 0;
        self.delay.fill(0.0);
    }
}

/// `<delay>`: frames, or seconds converted with the channel step.
fn delay_frames(el: &Element, ctx: &mut LoadContext<'_>, dt: f64) -> Result<usize, Error> {
    let delay_time =
        match Parameter::number_or_property(el.data_line(0), ctx.tree(), el.location())? {
            Parameter::Property(p) => {
                let node = p.node(ctx.tree())?;
                if ctx.tree().is_tied(node) {
                    return Err(Error::Unsupported {
                        construct: "<delay> read from a tied property".to_owned(),
                        path: el.location().to_string(),
                    });
                }
                ctx.tree().get_double(node) * p.sign()
            }
            Parameter::Real(v) => v,
            _ => 0.0,
        };
    match el.attribute("type") {
        "" | "time" => unsigned_frames(el, delay_time / dt),
        "frames" => unsigned_frames(el, delay_time),
        _ => {
            lenient(el, ctx, "Unallowed delay type")?;
            Ok(0)
        }
    }
}

/// `(unsigned int)frames`, refusing the values for which C++'s conversion
/// is undefined.
fn unsigned_frames(el: &Element, frames: f64) -> Result<usize, Error> {
    // Defined when the truncated value fits in an unsigned int.
    if !(frames > -1.0 && frames < f64::from(u32::MAX) + 1.0) {
        return Err(model_error(el, format!("Invalid delay of {frames} frames")));
    }
    Ok(frames as u32 as usize)
}

/// `<clipto>`: both limits are needed for clipping to apply.
fn load_clip(el: &Element, ctx: &mut LoadContext<'_>) -> Result<Option<Clip>, Error> {
    let Some(clip_el) = el.find_element("clipto") else {
        return Ok(None);
    };
    let Some(min_el) = clip_el.find_element("min") else {
        lenient(
            clip_el,
            ctx,
            "Element <min> is missing, <clipto> is ignored.",
        )?;
        return Ok(None);
    };
    let min = parameter_value(min_el, ctx)?;
    let Some(max_el) = clip_el.find_element("max") else {
        lenient(
            clip_el,
            ctx,
            "Element <max> is missing, <clipto> is ignored.",
        )?;
        return Ok(None);
    };
    let max = parameter_value(max_el, ctx)?;
    Ok(Some(Clip {
        min,
        max,
        cyclic: clip_el.attribute("type") == "cyclic",
    }))
}
