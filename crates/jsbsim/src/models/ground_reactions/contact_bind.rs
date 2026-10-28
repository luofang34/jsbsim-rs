// SPDX-License-Identifier: LGPL-2.1-or-later
// FGLGear: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/FGLGear.cpp (bind) and
// src/FGJSBBase.cpp (CreateIndexedPropertyName).
//
//! `FGLGear::bind`: a contact's properties, tied as it is loaded.

use super::contact::{Contact, ContactType, SteerType};
use crate::Error;
use crate::fdm::{Binder, Kind, Models};
use crate::math::{E_X, E_Y, E_Z};

type Get = fn(&Contact) -> f64;
type Set = fn(&mut Contact, f64);

/// A property under the contact's base name: suffix, type, getter and
/// setter (none for a read-only tie).
type Entry = (&'static str, Kind, Get, Option<Set>);

const D: Kind = Kind::Double;

/// `CreateIndexedPropertyName(property, index)`.
#[must_use]
pub fn indexed_name(property: &str, index: usize) -> String {
    format!("{property}[{index}]")
}

fn contact(m: &Models, index: usize) -> Option<&Contact> {
    m.ground_reactions.contacts.get(index)
}

fn tie(
    b: &mut Binder<'_>,
    index: usize,
    path: &str,
    kind: Kind,
    get: Get,
    set: Option<Set>,
) -> Result<(), Error> {
    let getter = move |m: &Models| contact(m, index).map_or(0.0, get);
    match set {
        Some(set) => b.property(path, kind, getter, move |m, v| {
            if let Some(c) = m.ground_reactions.contacts.get_mut(index) {
                set(c, v);
            }
        })?,
        None => b.typed_getter(path, kind, getter)?,
    };
    Ok(())
}

fn from_bool(v: bool) -> f64 {
    f64::from(u8::from(v))
}

/// The properties every contact has, before the type-specific ones.
const COMMON: [Entry; 9] = [
    ("AGL-ft", D, |c| c.state.agl, Some(|c, v| c.state.agl = v)),
    (
        "WOW",
        Kind::Bool,
        |c| from_bool(c.state.wow),
        Some(|c, v| c.state.wow = v != 0.0),
    ),
    (
        "x-position",
        D,
        |c| c.location[E_X],
        Some(|c, v| {
            c.location[E_X] = v;
            c.acting_location[E_X] = v;
        }),
    ),
    (
        "y-position",
        D,
        |c| c.location[E_Y],
        Some(|c, v| {
            c.location[E_Y] = v;
            c.acting_location[E_Y] = v;
        }),
    ),
    (
        "z-position",
        D,
        |c| c.location[E_Z],
        Some(|c, v| {
            c.location[E_Z] = v;
            c.acting_location[E_Z] = v;
        }),
    ),
    (
        "compression-ft",
        D,
        |c| c.state.compress_length,
        Some(|c, v| c.state.compress_length = v),
    ),
    (
        "compression-velocity-fps",
        D,
        |c| c.state.compress_speed,
        Some(|c, v| c.state.compress_speed = v),
    ),
    (
        "static_friction_coeff",
        D,
        |c| c.friction.static_coeff,
        Some(Contact::set_static_coeff),
    ),
    (
        "dynamic_friction_coeff",
        D,
        |c| c.friction.dynamic_coeff,
        Some(|c, v| c.friction.dynamic_coeff = v),
    ),
];

/// BOGEY only.
const BOGEY: [Entry; 4] = [
    (
        "slip-angle-deg",
        D,
        |c| c.state.wheel_slip,
        Some(|c, v| c.state.wheel_slip = v),
    ),
    ("wheel-speed-fps", D, Contact::wheel_roll_vel, None),
    (
        "side_friction_coeff",
        D,
        |c| c.state.side_coeff,
        Some(|c, v| c.state.side_coeff = v),
    ),
    (
        "rolling_friction_coeff",
        D,
        |c| c.friction.rolling_coeff,
        Some(|c, v| c.friction.rolling_coeff = v),
    ),
];

/// A castering BOGEY only.
const CASTER: [Entry; 2] = [
    ("steering-angle-deg", D, Contact::steer_angle_deg, None),
    (
        "castered",
        Kind::Bool,
        |c| from_bool(c.steering.castered),
        Some(|c, v| c.steering.castered = v != 0.0),
    ),
];

/// A retractable contact only.
const RETRACTABLE: [Entry; 1] = [("pos-norm", D, |c| c.gear_pos, Some(|c, v| c.gear_pos = v))];

/// The per-contact copies of the surface properties, last.
const SURFACE: [Entry; 5] = [
    (
        "solid",
        Kind::Bool,
        |c| from_bool(c.surface.solid),
        Some(|c, v| c.surface.solid = v != 0.0),
    ),
    (
        "bumpiness",
        D,
        |c| c.surface.bumpiness,
        Some(|c, v| c.surface.bumpiness = v),
    ),
    (
        "maximum-force-lbs",
        D,
        |c| c.surface.maximum_force,
        Some(|c, v| c.surface.maximum_force = v),
    ),
    (
        "rolling_friction-factor",
        D,
        |c| c.surface.rolling_factor,
        Some(|c, v| c.surface.rolling_factor = v),
    ),
    (
        "static-friction-factor",
        D,
        |c| c.surface.static_factor,
        Some(|c, v| c.surface.static_factor = v),
    ),
];

fn tie_all(b: &mut Binder<'_>, n: usize, base: &str, entries: &[Entry]) -> Result<(), Error> {
    for &(suffix, kind, get, set) in entries {
        tie(b, n, &format!("{base}/{suffix}"), kind, get, set)?;
    }
    Ok(())
}

/// `FGLGear::bind` for `c`, the contact that will sit at index
/// `c.number`.
pub fn bind(b: &mut Binder<'_>, c: &Contact) -> Result<(), Error> {
    let n = c.number;
    let base = match c.kind {
        ContactType::Bogey => indexed_name("gear/unit", n),
        ContactType::Structure => indexed_name("contact/unit", n),
    };
    tie_all(b, n, &base, &COMMON)?;
    if c.kind == ContactType::Bogey {
        tie_all(b, n, &base, &BOGEY)?;
        if c.steering.kind == SteerType::Caster {
            tie_all(b, n, &base, &CASTER)?;
        }
    }
    if c.retractable {
        tie_all(b, n, &base, &RETRACTABLE)?;
    }
    if c.steering.kind != SteerType::Fixed {
        // Outside the base name: JSBSim keeps the fcs/ prefix for backward
        // compatibility.
        let path = indexed_name("fcs/steer-pos-deg", n);
        let set: Set = Contact::set_steer_angle_deg;
        tie(b, n, &path, D, Contact::steer_angle_deg, Some(set))?;
    }
    tie_all(b, n, &base, &SURFACE)
}
