// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2005 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGSensor.cpp (bind).
//
//! The properties every sensor ties: its failures, its random seed and
//! its quantized value.

use super::Signal;
use crate::Error;
use crate::fdm::{self, LoadContext, Models};
use crate::models::fcs::Kind;
use crate::models::fcs::component::{Base, model_error, own_property};
use crate::models::fcs::families::{self, ComponentAt};
use crate::properties::mk_property_name;
use crate::xml::Element;

/// The signal of the sensor at `at`.
fn signal(m: &Models, at: ComponentAt) -> Option<&Signal> {
    let c = m.fcs.channels.get(at.channel)?.components.get(at.index)?;
    match &c.kind {
        Kind::Sensor(s) => Some(&s.signal),
        Kind::Accelerometer(s) => Some(&s.signal),
        Kind::Gyro(s) => Some(&s.signal),
        Kind::Magnetometer(s) => Some(&s.signal),
        _ => None,
    }
}

/// See [`signal`].
fn signal_mut(m: &mut Models, at: ComponentAt) -> Option<&mut Signal> {
    match &mut families::component_mut(&mut m.fcs, at)?.kind {
        Kind::Sensor(s) => Some(&mut s.signal),
        Kind::Accelerometer(s) => Some(&mut s.signal),
        Kind::Gyro(s) => Some(&mut s.signal),
        Kind::Magnetometer(s) => Some(&mut s.signal),
        _ => None,
    }
}

/// A failure flag, tied as a `double`: 1 when set; a positive write sets
/// it.
fn tie_flag(
    ctx: &mut LoadContext<'_>,
    path: &str,
    at: ComponentAt,
    get: fn(&Signal) -> bool,
    set: fn(&mut Signal, bool),
) -> Result<(), Error> {
    ctx.binder.property(
        path,
        fdm::Kind::Double,
        move |m| match signal(m, at).is_some_and(get) {
            true => 1.0,
            false => 0.0,
        },
        move |m, v| {
            if let Some(s) = signal_mut(m, at) {
                set(s, v > 0.0);
            }
        },
    )?;
    Ok(())
}

/// `FGSensor::bind`.
pub(super) fn bind(
    el: &Element,
    base: &mut Base,
    quant_property: &str,
    ctx: &mut LoadContext<'_>,
    at: ComponentAt,
) -> Result<(), Error> {
    base.bind(ctx)?;
    let tmp = own_property(&base.name);
    tie_flag(
        ctx,
        &format!("{tmp}/malfunction/fail_low"),
        at,
        |s| s.fail_low,
        |s, v| s.fail_low = v,
    )?;
    tie_flag(
        ctx,
        &format!("{tmp}/malfunction/fail_high"),
        at,
        |s| s.fail_high,
        |s, v| s.fail_high = v,
    )?;
    tie_flag(
        ctx,
        &format!("{tmp}/malfunction/fail_stuck"),
        at,
        |s| s.fail_stuck,
        |s, v| s.fail_stuck = v,
    )?;
    ctx.binder.property(
        &format!("{tmp}/randomseed"),
        fdm::Kind::Int,
        move |m| {
            let own = signal(m, at).and_then(|s| s.random_seed);
            f64::from(own.unwrap_or(m.exec.random_seed))
        },
        move |m, v| {
            if let Some(s) = signal_mut(m, at) {
                s.set_random_seed(v as i32);
            }
        },
    )?;
    // A name holding a '/' is not tied at all.
    if quant_property.is_empty() || quant_property.contains('/') {
        return Ok(());
    }
    let qprop = format!("fcs/{}", mk_property_name(quant_property, true));
    let tree = ctx.tree();
    let node = tree.get_or_create(tree.root(), &qprop)?;
    if tree.is_tied(node) {
        return Err(model_error(
            el,
            format!("Property {tmp} has already been successfully bound (late)."),
        ));
    }
    ctx.binder.typed_getter(&qprop, fdm::Kind::Int, move |m| {
        f64::from(signal(m, at).map_or(0, |s| s.quantized))
    })?;
    Ok(())
}
