// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGThruster.cpp (the
// constructor's ties).
//
//! The `propulsion/engine[n]/` properties of every thruster.

use crate::Error;
use crate::fdm::{Binder, Kind, Models};
use crate::math::{Axis, E_PITCH, E_X, E_Y, E_YAW, E_Z};
use crate::models::propulsion::ThrusterBase;

fn base(m: &Models, n: usize) -> Option<&ThrusterBase> {
    m.propulsion.engines.get(n).map(|e| &e.thruster.base)
}

fn base_mut(m: &mut Models, n: usize) -> Option<&mut ThrusterBase> {
    m.propulsion
        .engines
        .get_mut(n)
        .map(|e| &mut e.thruster.base)
}

/// The reference (`<location>`) and acting positions, in.
pub fn bind_location(b: &mut Binder<'_>, n: usize) -> Result<(), Error> {
    let axes: [(&str, Axis); 3] = [("x", E_X), ("y", E_Y), ("z", E_Z)];
    for (name, i) in axes {
        b.getter(
            &format!("propulsion/engine[{n}]/{name}-reference-position"),
            move |m| base(m, n).map_or(0.0, |t| t.force.location[i]),
        )?;
    }
    for (name, i) in axes {
        b.property(
            &format!("propulsion/engine[{n}]/{name}-position"),
            Kind::Double,
            move |m| base(m, n).map_or(0.0, |t| t.force.acting_location[i]),
            move |m, v| {
                if let Some(t) = base_mut(m, n) {
                    t.force.acting_location[i] = v;
                }
            },
        )?;
    }
    Ok(())
}

/// The pitch and yaw of the orientation, rad.
pub fn bind_orientation(b: &mut Binder<'_>, n: usize) -> Result<(), Error> {
    b.property(
        &format!("propulsion/engine[{n}]/pitch-angle-rad"),
        Kind::Double,
        move |m| base(m, n).map_or(0.0, |t| t.force.orient[E_PITCH]),
        move |m, v| {
            if let Some(t) = base_mut(m, n) {
                t.force.set_pitch(v);
            }
        },
    )?;
    b.property(
        &format!("propulsion/engine[{n}]/yaw-angle-rad"),
        Kind::Double,
        move |m| base(m, n).map_or(0.0, |t| t.force.orient[E_YAW]),
        move |m, v| {
            if let Some(t) = base_mut(m, n) {
                t.force.set_yaw(v);
            }
        },
    )?;
    Ok(())
}

/// The reverser angle of a direct thruster, rad.
pub fn bind_reverser(b: &mut Binder<'_>, n: usize) -> Result<(), Error> {
    b.property(
        &format!("propulsion/engine[{n}]/reverser-angle-rad"),
        Kind::Double,
        move |m| base(m, n).map_or(0.0, |t| t.reverser_angle),
        move |m, v| {
            if let Some(t) = base_mut(m, n) {
                t.reverser_angle = v;
            }
        },
    )?;
    Ok(())
}
