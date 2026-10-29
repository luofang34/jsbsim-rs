// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2003 David Culp (daveculp@cox.net)
// Derived from JSBSim src/models/propulsion/FGTurbine.cpp (bindmodel).
//
//! The `propulsion/engine[n]/` properties of a turbine engine.

use super::Turbine;
use crate::Error;
use crate::fdm::{Binder, Kind, Models};
use crate::models::propulsion::families::EngineKind;

fn turbine(m: &Models, n: usize) -> Option<&Turbine> {
    match &m.propulsion.engines.get(n)?.kind {
        EngineKind::Turbine(t) => Some(t),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

fn turbine_mut(m: &mut Models, n: usize) -> Option<&mut Turbine> {
    match &mut m.propulsion.engines.get_mut(n)?.kind {
        EngineKind::Turbine(t) => Some(t),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

type Get = fn(&Turbine) -> f64;
type Set = fn(&mut Turbine, f64);

fn flag(b: bool) -> f64 {
    f64::from(u8::from(b))
}

/// The read-write ties of `bindmodel`, in its order.
const MEMBERS: [(&str, Kind, Get, Set); 12] = [
    ("n1", Kind::Double, |t| t.state.n1, |t, v| t.state.n1 = v),
    ("n2", Kind::Double, |t| t.state.n2, |t, v| t.state.n2 = v),
    (
        "injection_cmd",
        Kind::Bool,
        |t| flag(t.flags.injection),
        |t, v| t.flags.injection = v != 0.0,
    ),
    (
        "seized",
        Kind::Bool,
        |t| flag(t.flags.seized),
        |t, v| t.flags.seized = v != 0.0,
    ),
    (
        "stalled",
        Kind::Bool,
        |t| flag(t.flags.stalled),
        |t, v| t.flags.stalled = v != 0.0,
    ),
    (
        "bleed-factor",
        Kind::Double,
        |t| t.state.bleed_demand,
        |t, v| t.state.bleed_demand = v,
    ),
    (
        "MaxN1",
        Kind::Double,
        |t| t.state.max_n1,
        |t, v| t.state.max_n1 = v,
    ),
    (
        "MaxN2",
        Kind::Double,
        |t| t.state.max_n2,
        |t, v| t.state.max_n2 = v,
    ),
    (
        "InjectionTimer",
        Kind::Double,
        |t| t.state.injection_timer,
        |t, v| t.state.injection_timer = v,
    ),
    (
        "InjWaterNorm",
        Kind::Double,
        |t| t.state.inj_water_norm,
        |t, v| t.state.inj_water_norm = v,
    ),
    (
        "InjN1increment",
        Kind::Double,
        |t| t.state.inj_n1_increment,
        |t, v| t.state.inj_n1_increment = v,
    ),
    (
        "InjN2increment",
        Kind::Double,
        |t| t.state.inj_n2_increment,
        |t, v| t.state.inj_n2_increment = v,
    ),
];

/// The ties of `bindmodel` for engine `n`.
pub fn bind(b: &mut Binder<'_>, n: usize) -> Result<(), Error> {
    let prefix = format!("propulsion/engine[{n}]");
    for (name, kind, get, set) in MEMBERS {
        b.property(
            &format!("{prefix}/{name}"),
            kind,
            move |m| turbine(m, n).map_or(0.0, get),
            move |m, v| {
                if let Some(t) = turbine_mut(m, n) {
                    set(t, v);
                }
            },
        )?;
    }
    b.getter(&format!("{prefix}/atsfc"), move |m| {
        turbine(m, n).map_or(0.0, |t| t.config.atsfc)
    })?;
    // A tie to the member with its WRITE attribute cleared.
    b.getter(&format!("{prefix}/tsfc"), move |m| {
        turbine(m, n).map_or(0.0, |t| t.state.corrected_tsfc)
    })?;
    Ok(())
}
