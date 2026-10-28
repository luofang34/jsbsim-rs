// SPDX-License-Identifier: LGPL-2.1-or-later
// FGFCS: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// FGFCSComponent, FGFCSFunction: Copyright (C) 2000-2005 Jon S. Berndt.
// Derived from JSBSim src/models/FGFCS.cpp (Load),
// src/models/flight_control/FGFCSComponent.cpp (constructor, bind) and
// FGFCSFunction.cpp.
//
//! Loading channels and their components.

use super::{Channel, Clip, Fcs, FcsFunction};
use crate::Error;
use crate::fdm::LoadContext;
use crate::functions::{Parameter, PropertyValue};
use crate::properties::mk_property_name;
use crate::xml::Element;

/// Component element names JSBSim knows; all but `fcs_function` are
/// still to be ported.
const COMPONENTS: &[&str] = &[
    "lag_filter",
    "lead_lag_filter",
    "washout_filter",
    "second_order_filter",
    "pure_gain",
    "scheduled_gain",
    "aerosurface_scale",
    "summer",
    "deadband",
    "switch",
    "kinematic",
    "pid",
    "integrator",
    "actuator",
    "sensor",
    "accelerometer",
    "magnetometer",
    "gyro",
    "waypoint_heading",
    "waypoint_distance",
    "angle",
    "distributor",
    "linear_actuator",
];

fn model_error(el: &Element, message: impl std::fmt::Display) -> Error {
    Error::Model {
        message: format!("{}: {message}", el.location()),
    }
}

/// `FGParameterValue(el)`: exactly one non-empty data line.
fn parameter_value(el: &Element, ctx: &mut LoadContext<'_>) -> Result<Parameter, Error> {
    let value = el.data_line(0);
    if el.data_lines().len() != 1 || value.is_empty() {
        return Err(model_error(
            el,
            format!(
                "The element <{}> must either contain a value number or a property name.",
                el.name()
            ),
        ));
    }
    Parameter::number_or_property(value, ctx.tree(), el.location())
}

impl Fcs {
    /// The channels of one `<flight_control>`, `<autopilot>` or `<system>`
    /// document, after the executive's `Upload`.
    pub fn load_channels(
        &mut self,
        document: &Element,
        ctx: &mut LoadContext<'_>,
    ) -> Result<(), Error> {
        for channel_el in document.find_elements("channel") {
            self.channel_rate = match channel_el.attribute("execrate") {
                "" => 1,
                _ => channel_el.attribute_as_number("execrate")? as i32,
            };
            let on_off = match channel_el.attribute("execute") {
                "" => None,
                property => Some(ctx.tree().get(property)?.ok_or_else(|| {
                    model_error(
                        channel_el,
                        format!(
                            "The On/Off property, {property} specified for channel {} is undefined or not understood. The simulation will abort",
                            channel_el.attribute("name")
                        ),
                    )
                })?),
            };
            let mut channel = Channel::new(
                channel_el.attribute("name").to_owned(),
                self.channel_rate,
                on_off,
            );
            for component in channel_el.children() {
                match component.name() {
                    "fcs_function" => channel.components.push(load_fcs_function(component, ctx)?),
                    name if COMPONENTS.contains(&name) => {
                        return Err(Error::Unsupported {
                            construct: format!("<{name}> FCS component"),
                            path: component.location().to_string(),
                        });
                    }
                    name => {
                        if ctx.options.strict {
                            return Err(Error::Unsupported {
                                construct: format!("unknown FCS component <{name}>"),
                                path: component.location().to_string(),
                            });
                        }
                        tracing::error!("{}: Unknown FCS component: {name}", component.location());
                    }
                }
            }
            self.channels.push(channel);
        }
        Ok(())
    }
}

/// `FGFCSComponent(fcs, el)` followed by `FGFCSFunction`'s constructor.
fn load_fcs_function(el: &Element, ctx: &mut LoadContext<'_>) -> Result<FcsFunction, Error> {
    let name = el.attribute("name").to_owned();
    for init in el.find_elements("init") {
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
    let clip = load_clip(el, ctx)?;
    let Some(function_el) = el.find_element("function") else {
        return Err(model_error(
            el,
            "FCS Function should contain a \"function\" element",
        ));
    };
    let function = ctx
        .functions
        .load_function(ctx.binder.tree, function_el, "")?;
    let own = if name.contains('/') {
        name.clone()
    } else {
        format!("fcs/{}", mk_property_name(&name, true))
    };
    outputs.push(output_node(&own, ctx)?);
    Ok(FcsFunction {
        name,
        inputs,
        outputs,
        function,
        clip,
        output: 0.0,
    })
}

/// An output node; a new node starts at the component's output, 0.
fn output_node(path: &str, ctx: &mut LoadContext<'_>) -> Result<crate::properties::NodeId, Error> {
    let tree = ctx.tree();
    let existed = tree.has_node(path)?;
    let node = tree.get_or_create(tree.root(), path)?;
    if !existed {
        tree.set_double(node, 0.0);
    }
    Ok(node)
}

/// `<clipto>`: both limits are needed for clipping to apply.
fn load_clip(el: &Element, ctx: &mut LoadContext<'_>) -> Result<Option<Clip>, Error> {
    let Some(clip_el) = el.find_element("clipto") else {
        return Ok(None);
    };
    let Some(min_el) = clip_el.find_element("min") else {
        return Ok(None);
    };
    let min = parameter_value(min_el, ctx)?;
    let Some(max_el) = clip_el.find_element("max") else {
        return Ok(None);
    };
    let max = parameter_value(max_el, ctx)?;
    Ok(Some(Clip {
        min,
        max,
        cyclic: clip_el.attribute("type") == "cyclic",
    }))
}
