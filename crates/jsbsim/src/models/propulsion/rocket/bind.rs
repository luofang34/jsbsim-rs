// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGRocket.cpp (bindmodel).
//
//! The `propulsion/engine[n]/` properties of a rocket engine.

use super::Rocket;
use crate::Error;
use crate::fdm::{Binder, Models};
use crate::models::propulsion::families::EngineKind;

fn rocket(m: &Models, n: usize) -> Option<&Rocket> {
    match &m.propulsion.engines.get(n)?.kind {
        EngineKind::Rocket(r) => Some(r),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

fn rocket_mut(m: &mut Models, n: usize) -> Option<&mut Rocket> {
    match &mut m.propulsion.engines.get_mut(n)?.kind {
        EngineKind::Rocket(r) => Some(r),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

type Get = fn(&Rocket) -> f64;
type Set = fn(&mut Rocket, f64);

/// Suffix, getter and setter (none for a read-only tie), in `bindmodel`'s
/// order: the ties of every rocket, then those of a liquid one, which
/// every rocket gets.
const PROPERTIES: [(&str, Get, Option<Set>); 6] = [
    ("total-impulse", |r| r.it, None),
    ("total-vac-impulse", |r| r.it_vac, None),
    ("vacuum-thrust_lbs", |r| r.vac_thrust, None),
    ("oxi-flow-rate-pps", |r| r.oxidizer_flow_rate, None),
    ("mixture-ratio", |r| r.mxr, Some(|r, v| r.mxr = v)),
    ("isp", |r| r.isp, Some(|r, v| r.isp = v)),
];

/// `FGRocket::bindmodel` for engine `n`.
pub fn bind(b: &mut Binder<'_>, n: usize) -> Result<(), Error> {
    for (suffix, get, set) in PROPERTIES {
        let path = format!("propulsion/engine[{n}]/{suffix}");
        let get = move |m: &Models| rocket(m, n).map_or(0.0, get);
        match set {
            Some(set) => {
                b.property(&path, crate::fdm::Kind::Double, get, move |m, v| {
                    if let Some(r) = rocket_mut(m, n) {
                        set(r, v);
                    }
                })?;
            }
            None => {
                b.getter(&path, get)?;
            }
        }
    }
    Ok(())
}
