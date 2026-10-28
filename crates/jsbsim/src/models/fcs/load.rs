// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/FGFCS.cpp (Load).
//
//! Loading channels and their components.

use super::component::{Base, model_error};
use super::families::{self, Component, ComponentAt};
use super::{Channel, Fcs};
use crate::Error;
use crate::fdm::LoadContext;
use crate::xml::Element;

/// Component element names `FGFCS::Load` knows. A name a family registers
/// loads that family; the others are refused as not ported yet.
const KNOWN: &[&str] = &[
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
    "fcs_function",
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

impl Fcs {
    /// The channels of one `<flight_control>`, `<autopilot>` or `<system>`
    /// document, after the executive's `Upload`; `exec_dt` is the
    /// executive's step at load.
    pub fn load_channels(
        &mut self,
        document: &Element,
        ctx: &mut LoadContext<'_>,
        exec_dt: f64,
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
            let dt = self.channel_delta_t(exec_dt);
            let at = self.channels.len();
            for el in channel_el.children() {
                let at = ComponentAt {
                    channel: at,
                    index: channel.components.len(),
                };
                if let Some(component) = load_component(el, ctx, dt, at)? {
                    channel.components.push(component);
                }
            }
            self.channels.push(channel);
        }
        Ok(())
    }
}

/// One child of a channel: its component, or `None` for an unknown
/// element JSBSim logs and skips.
fn load_component(
    el: &Element,
    ctx: &mut LoadContext<'_>,
    dt: f64,
    at: ComponentAt,
) -> Result<Option<Component>, Error> {
    let name = el.name();
    if !KNOWN.contains(&name) {
        if ctx.options.strict {
            return Err(Error::Unsupported {
                construct: format!("unknown FCS component <{name}>"),
                path: el.location().to_string(),
            });
        }
        tracing::error!("{}: Unknown FCS component: {name}", el.location());
        return Ok(None);
    }
    if !families::registers(name) {
        return Err(Error::Unsupported {
            construct: format!("<{name}> FCS component"),
            path: el.location().to_string(),
        });
    }
    let mut base = Base::load(el, ctx, dt)?;
    let kind = families::load(el, &mut base, ctx, at)?;
    Ok(Some(Component { base, kind }))
}
