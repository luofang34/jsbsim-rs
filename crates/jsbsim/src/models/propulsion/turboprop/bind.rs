// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2004 Jiri "Javky" Javurek (javky@email.cz)
// Derived from JSBSim src/models/propulsion/FGTurboProp.cpp (bindmodel).
//
//! The `propulsion/engine[n]/` properties of a turboprop engine.

use super::TurboProp;
use crate::Error;
use crate::fdm::{Binder, Kind, Models};
use crate::models::propulsion::families::EngineKind;

fn turboprop(m: &Models, n: usize) -> Option<&TurboProp> {
    match &m.propulsion.engines.get(n)?.kind {
        EngineKind::TurboProp(t) => Some(t),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

fn turboprop_mut(m: &mut Models, n: usize) -> Option<&mut TurboProp> {
    match &mut m.propulsion.engines.get_mut(n)?.kind {
        EngineKind::TurboProp(t) => Some(t),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

type Get = fn(&TurboProp) -> f64;
type Set = fn(&mut TurboProp, f64);

fn flag(b: bool) -> f64 {
    f64::from(u8::from(b))
}

/// The ties of `bindmodel`, each to a member, in its order.
const MEMBERS: [(&str, Kind, Get, Set); 7] = [
    ("n1", Kind::Double, |t| t.state.n1, |t, v| t.state.n1 = v),
    (
        "reverser",
        Kind::Bool,
        |t| flag(t.state.reversed),
        |t, v| t.state.reversed = v != 0.0,
    ),
    (
        "power-hp",
        Kind::Double,
        |t| t.state.hp,
        |t, v| t.state.hp = v,
    ),
    (
        "itt-c",
        Kind::Double,
        |t| t.state.eng_itt_degc,
        |t, v| t.state.eng_itt_degc = v,
    ),
    (
        "engtemp-c",
        Kind::Double,
        |t| t.state.eng_temperature,
        |t, v| t.state.eng_temperature = v,
    ),
    (
        "ielu_intervent",
        Kind::Bool,
        |t| flag(t.state.ielu_intervent),
        |t, v| t.state.ielu_intervent = v != 0.0,
    ),
    (
        "combustion_efficiency",
        Kind::Double,
        |t| t.state.combustion_efficiency,
        |t, v| t.state.combustion_efficiency = v,
    ),
];

/// The ties of `bindmodel` for engine `n`.
pub fn bind(b: &mut Binder<'_>, n: usize) -> Result<(), Error> {
    for (name, kind, get, set) in MEMBERS {
        b.property(
            &format!("propulsion/engine[{n}]/{name}"),
            kind,
            move |m| turboprop(m, n).map_or(0.0, get),
            move |m, v| {
                if let Some(t) = turboprop_mut(m, n) {
                    set(t, v);
                }
            },
        )?;
    }
    Ok(())
}
