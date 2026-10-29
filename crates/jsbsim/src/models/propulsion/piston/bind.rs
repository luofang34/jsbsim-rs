// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGPiston.cpp (the
// constructor's ties).
//
//! The `propulsion/engine[n]/` properties of a piston engine.

use super::Piston;
use crate::Error;
use crate::fdm::{Binder, Kind, Models};
use crate::models::propulsion::families::EngineKind;

fn piston(m: &Models, n: usize) -> Option<&Piston> {
    match &m.propulsion.engines.get(n)?.kind {
        EngineKind::Piston(p) => Some(p),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

fn piston_mut(m: &mut Models, n: usize) -> Option<&mut Piston> {
    match &mut m.propulsion.engines.get_mut(n)?.kind {
        EngineKind::Piston(p) => Some(p),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

type Get = fn(&Piston) -> f64;
type Set = fn(&mut Piston, f64);

/// A `Tie(name, &member)`: read and written as `kind`.
fn member(
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
        move |m| piston(m, n).map_or(0.0, get),
        move |m, v| {
            if let Some(p) = piston_mut(m, n) {
                set(p, v);
            }
        },
    )?;
    Ok(())
}

fn getter(b: &mut Binder<'_>, n: usize, name: &str, get: Get) -> Result<(), Error> {
    b.getter(&format!("propulsion/engine[{n}]/{name}"), move |m| {
        piston(m, n).map_or(0.0, get)
    })?;
    Ok(())
}

/// The `Tie(name, &member)`s, in `FGPiston`'s order.
const MEMBERS: [(&str, Kind, Get, Set); 11] = [
    (
        "power-hp",
        Kind::Double,
        |p| p.state.hp,
        |p, v| p.state.hp = v,
    ),
    (
        "friction-hp",
        Kind::Double,
        |p| p.tuning.static_friction_hp,
        |p, v| p.tuning.static_friction_hp = v,
    ),
    (
        "bsfc-lbs_hphr",
        Kind::Double,
        |p| p.tuning.isfc,
        |p, v| p.tuning.isfc = v,
    ),
    (
        "starter-norm",
        Kind::Double,
        |p| p.tuning.starter_gain,
        |p, v| p.tuning.starter_gain = v,
    ),
    (
        "volumetric-efficiency",
        Kind::Double,
        |p| p.tuning.volumetric_efficiency,
        |p, v| p.tuning.volumetric_efficiency = v,
    ),
    (
        "map-pa",
        Kind::Double,
        |p| p.state.map,
        |p, v| p.state.map = v,
    ),
    (
        "map-inhg",
        Kind::Double,
        |p| p.state.manifold_pressure_inhg,
        |p, v| p.state.manifold_pressure_inhg = v,
    ),
    (
        "air-intake-impedance-factor",
        Kind::Double,
        |p| p.tuning.z_airbox,
        |p, v| p.tuning.z_airbox = v,
    ),
    (
        "ram-air-factor",
        Kind::Double,
        |p| p.tuning.ram_air_factor,
        |p, v| p.tuning.ram_air_factor = v,
    ),
    (
        "cooling-factor",
        Kind::Double,
        |p| p.tuning.cooling_factor,
        |p, v| p.tuning.cooling_factor = v,
    ),
    (
        "boost-speed",
        Kind::Int,
        |p| f64::from(p.boost.speed),
        |p, v| p.boost.speed = v as i32,
    ),
];

/// The supercharger loss's ties.
const BOOST_LOSS: [(&str, Kind, Get, Set); 2] = [
    (
        "boostloss-factor",
        Kind::Double,
        |p| p.boost.loss_factor,
        |p, v| p.boost.loss_factor = v,
    ),
    (
        "boostloss-hp",
        Kind::Double,
        |p| p.boost.loss_hp,
        |p, v| p.boost.loss_hp = v,
    ),
];

/// The ties of `FGPiston`'s constructor for engine `n`; the supercharger
/// loss's only when it has a loss factor.
pub fn bind(b: &mut Binder<'_>, n: usize, boost_loss: bool) -> Result<(), Error> {
    for (name, kind, get, set) in MEMBERS {
        member(b, n, name, kind, get, set)?;
    }
    getter(b, n, "cht-degF", Piston::cylinder_head_temp_degf)?;
    getter(b, n, "oil-temperature-degF", Piston::oil_temp_degf)?;
    getter(b, n, "oil-pressure-psi", |p| p.state.oil_pressure_psi)?;
    getter(b, n, "egt-degF", Piston::exhaust_gas_temp_degf)?;
    if boost_loss {
        for (name, kind, get, set) in BOOST_LOSS {
            member(b, n, name, kind, get, set)?;
        }
    }
    getter(b, n, "AFR", Piston::afr)?;
    Ok(())
}
