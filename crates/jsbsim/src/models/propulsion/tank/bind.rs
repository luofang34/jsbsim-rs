// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGTank.cpp (bind).
//
//! Properties of a tank, under `propulsion/tank[n]/`, in JSBSim's order.

use super::Tank;
use crate::Error;
use crate::fdm::{Binder, Kind, Models};
use crate::math::{E_X, E_Y, E_Z};

type Get = fn(&Tank) -> f64;
type Set = fn(&mut Tank, f64);

/// Suffix, type, getter and setter (none for a read-only tie).
const PROPERTIES: [(&str, Kind, Get, Option<Set>); 12] = [
    (
        "contents-lbs",
        Kind::Double,
        |t| t.contents,
        Some(|t, v| t.set_contents(v)),
    ),
    (
        "unusable-volume-gal",
        Kind::Double,
        |t| t.unusable_volume,
        Some(|t, v| t.unusable_volume = v),
    ),
    ("pct-full", Kind::Double, |t| t.pct_full, None),
    ("density-lbs_per_gal", Kind::Double, |t| t.density, None),
    (
        "priority",
        Kind::Int,
        |t| f64::from(t.priority),
        Some(|t, v| t.set_priority(v as i32)),
    ),
    (
        "external-flow-rate-pps",
        Kind::Double,
        |t| t.external_flow,
        Some(|t, v| t.external_flow = v),
    ),
    ("local-ixx-slug_ft2", Kind::Double, |t| t.ixx, None),
    ("local-iyy-slug_ft2", Kind::Double, |t| t.iyy, None),
    ("local-izz-slug_ft2", Kind::Double, |t| t.izz, None),
    (
        "x-position",
        Kind::Double,
        |t| t.xyz[E_X],
        Some(|t, v| t.xyz[E_X] = v),
    ),
    (
        "y-position",
        Kind::Double,
        |t| t.xyz[E_Y],
        Some(|t, v| t.xyz[E_Y] = v),
    ),
    (
        "z-position",
        Kind::Double,
        |t| t.xyz[E_Z],
        Some(|t, v| t.xyz[E_Z] = v),
    ),
];

fn tank(m: &Models, n: usize) -> Option<&Tank> {
    m.propulsion.tanks.get(n)
}

/// `FGTank::bind` for tank `n`.
pub fn bind(b: &mut Binder<'_>, n: usize) -> Result<(), Error> {
    for (suffix, kind, get, set) in PROPERTIES {
        let path = format!("propulsion/tank[{n}]/{suffix}");
        let get = move |m: &Models| tank(m, n).map_or(0.0, get);
        match set {
            Some(set) => {
                b.property(&path, kind, get, move |m, v| {
                    if let Some(t) = m.propulsion.tanks.get_mut(n) {
                        set(t, v);
                    }
                })?;
            }
            None => {
                b.typed_getter(&path, kind, get)?;
            }
        }
    }
    Ok(())
}
