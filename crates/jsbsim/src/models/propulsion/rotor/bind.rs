// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2010 T. Kreitler (t.kreitler@web.de)
// Derived from JSBSim src/models/propulsion/FGRotor.cpp (bindmodel).
//
//! The `propulsion/engine[n]/` properties of a rotor, and the property its
//! external speed is read from.

use super::{ControlMap, Rotor, RpmDrive};
use crate::Error;
use crate::fdm::{Binder, Kind, LoadContext, Models};
use crate::models::propulsion::families::ThrusterKind;

fn rotor(m: &Models, n: usize) -> Option<&Rotor> {
    match &m.propulsion.engines.get(n)?.thruster.kind {
        ThrusterKind::Rotor(r) => Some(r),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

fn rotor_mut(m: &mut Models, n: usize) -> Option<&mut Rotor> {
    match &mut m.propulsion.engines.get_mut(n)?.thruster.kind {
        ThrusterKind::Rotor(r) => Some(r),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

type Get = fn(&Rotor) -> f64;
type Set = fn(&mut Rotor, f64);

/// The read-only ties, in `bindmodel`'s order.
const STATE: [(&str, Get); 13] = [
    ("rotor-rpm", |r| r.state.rpm),
    ("engine-rpm", |r| r.state.engine_rpm),
    ("a0-rad", |r| r.state.a0),
    ("a1-rad", |r| r.state.a1s),
    ("b1-rad", |r| r.state.b1s),
    ("inflow-ratio", |r| r.state.lambda),
    ("advance-ratio", |r| r.state.mu),
    ("induced-inflow-ratio", |r| r.state.nu),
    ("vi-fps", |r| r.state.v_induced),
    ("thrust-coefficient", |r| r.state.c_t),
    ("torque-lbsft", |r| r.state.torque),
    ("theta-downwash-rad", |r| r.state.theta_downwash),
    ("phi-downwash-rad", |r| r.state.phi_downwash),
];

const GROUND_EFFECT: (&str, Get, Set) = (
    "groundeffect-scale-norm",
    |r| r.controls.ground_effect_scale_norm,
    |r, v| r.controls.ground_effect_scale_norm = v,
);
const COLLECTIVE: (Get, Set) = (|r| r.controls.collective, |r, v| r.controls.collective = v);
const LATERAL: (&str, Get, Set) = (
    "lateral-ctrl-rad",
    |r| r.controls.lateral,
    |r, v| r.controls.lateral = v,
);
const LONGITUDINAL: (&str, Get, Set) = (
    "longitudinal-ctrl-rad",
    |r| r.controls.longitudinal,
    |r, v| r.controls.longitudinal = v,
);

fn read_write(
    b: &mut Binder<'_>,
    n: usize,
    (name, get, set): (&str, Get, Set),
) -> Result<(), Error> {
    b.property(
        &format!("propulsion/engine[{n}]/{name}"),
        Kind::Double,
        move |m| rotor(m, n).map_or(0.0, get),
        move |m, v| {
            if let Some(r) = rotor_mut(m, n) {
                set(r, v);
            }
        },
    )?;
    Ok(())
}

/// `FGRotor::bindmodel` for the rotor of engine `n`: its state, the
/// controls its map names, and the node an external speed is read from
/// (`x-rpm-dict`, made if missing, or the source engine's `rotor-rpm`,
/// which must exist already).
pub fn bind(this: &mut Rotor, ctx: &mut LoadContext<'_>, n: usize) -> Result<(), Error> {
    let b = &mut ctx.binder;
    for (name, get) in STATE {
        b.getter(&format!("propulsion/engine[{n}]/{name}"), move |m| {
            rotor(m, n).map_or(0.0, get)
        })?;
    }
    read_write(b, n, GROUND_EFFECT)?;
    let (get, set) = COLLECTIVE;
    match this.controls.map {
        ControlMap::Tail => read_write(b, n, ("antitorque-ctrl-rad", get, set))?,
        ControlMap::Tandem => {
            read_write(b, n, ("tail-collective-ctrl-rad", get, set))?;
            read_write(b, n, LATERAL)?;
            read_write(b, n, LONGITUDINAL)?;
        }
        ControlMap::Main => {
            read_write(b, n, ("collective-ctrl-rad", get, set))?;
            read_write(b, n, LATERAL)?;
            read_write(b, n, LONGITUDINAL)?;
        }
    }
    let RpmDrive::External(ext) = &mut this.drive else {
        return Ok(());
    };
    let tree = ctx.tree();
    if ext.definition == -1 {
        let path = format!("propulsion/engine[{n}]/x-rpm-dict");
        ext.source = Some(tree.get_or_create(tree.root(), &path)?);
        return Ok(());
    }
    match usize::try_from(ext.definition) {
        Ok(source) if source != n => {
            let path = format!("propulsion/engine[{source}]/rotor-rpm");
            ext.source = tree.get(&path)?;
            if ext.source.is_none() {
                tracing::error!(
                    "# Warning: Engine number {n}.\n# No 'rotor-rpm' property found for engine {source}.\n# Please check order of engine definitons."
                );
            }
        }
        _ => {
            tracing::error!(
                "# Engine number {n}, given ExternalRPM value '{}' unhandled.",
                ext.definition
            );
        }
    }
    Ok(())
}
