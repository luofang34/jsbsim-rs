// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGPropeller.cpp (the
// constructor's ties).
//
//! The `propulsion/engine[n]/` properties of a propeller.

use super::Propeller;
use crate::Error;
use crate::fdm::{Binder, Kind, Models};
use crate::math::E_X;
use crate::models::propulsion::families::ThrusterKind;
use crate::models::propulsion::thruster::ThrusterBase;

fn propeller(m: &Models, n: usize) -> Option<(&ThrusterBase, &Propeller)> {
    let thruster = &m.propulsion.engines.get(n)?.thruster;
    match &thruster.kind {
        ThrusterKind::Propeller(p) => Some((&thruster.base, p)),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

fn propeller_mut(m: &mut Models, n: usize) -> Option<(&mut ThrusterBase, &mut Propeller)> {
    let thruster = &mut m.propulsion.engines.get_mut(n)?.thruster;
    match &mut thruster.kind {
        ThrusterKind::Propeller(p) => Some((&mut thruster.base, p)),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

type Get = fn(&ThrusterBase, &Propeller) -> f64;
type Set = fn(&mut ThrusterBase, &mut Propeller, f64);

fn getter(b: &mut Binder<'_>, n: usize, name: &str, get: Get) -> Result<(), Error> {
    b.getter(&format!("propulsion/engine[{n}]/{name}"), move |m| {
        propeller(m, n).map_or(0.0, |(t, p)| get(t, p))
    })?;
    Ok(())
}

fn property(
    b: &mut Binder<'_>,
    n: usize,
    name: &str,
    kind: Kind,
    get: Get,
    set: Set,
) -> Result<(), Error> {
    b.property(
        &format!("propulsion/engine[{n}]/{name}"),
        kind,
        move |m| propeller(m, n).map_or(0.0, |(t, p)| get(t, p)),
        move |m, v| {
            if let Some((t, p)) = propeller_mut(m, n) {
                set(t, p, v);
            }
        },
    )?;
    Ok(())
}

/// The ties of `FGPropeller`'s constructor for engine `n`.
pub fn bind(b: &mut Binder<'_>, n: usize) -> Result<(), Error> {
    let double = Kind::Double;
    getter(b, n, "engine-rpm", |t, p| p.rpm * t.gear_ratio)?;
    property(b, n, "advance-ratio", double, |_, p| p.j, |_, p, v| p.j = v)?;
    property(
        b,
        n,
        "blade-angle",
        double,
        |_, p| p.pitch,
        |_, p, v| p.pitch = v,
    )?;
    getter(b, n, "thrust-coefficient", |t, _| t.thrust_coeff)?;
    getter(b, n, "propeller-rpm", |_, p| p.rpm)?;
    getter(b, n, "helical-tip-Mach", |_, p| p.helical_tip_mach)?;
    property(
        b,
        n,
        "constant-speed-mode",
        Kind::Int,
        |_, p| f64::from(p.constant_speed),
        |_, p, v| p.constant_speed = v as i32,
    )?;
    property(
        b,
        n,
        "prop-induced-velocity_fps",
        double,
        |_, p| p.vinduced,
        |_, p, v| p.vinduced = v,
    )?;
    property(
        b,
        n,
        "propeller-power-ftlbps",
        double,
        |t, _| t.power_required,
        |t, _, v| t.power_required = v,
    )?;
    getter(b, n, "propeller-torque-ftlb", |_, p| p.torque[E_X])?;
    property(
        b,
        n,
        "propeller-sense",
        double,
        |_, p| p.sense,
        |_, p, v| p.sense = v,
    )?;
    Ok(())
}
