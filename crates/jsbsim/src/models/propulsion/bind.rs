// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGPropulsion.cpp (bind) and
// src/models/propulsion/FGEngine.cpp (Load's ties).
//
//! Properties of the propulsion system, which JSBSim ties only once it has
//! loaded an engine, and of each engine.

use super::EngineType;
use crate::Error;
use crate::fdm::{Binder, ExecRequest, Kind, Models};
use crate::math::{E_X, E_Y, E_Z};

fn flag(b: bool) -> f64 {
    f64::from(u8::from(b))
}

/// `FGPropulsion::bind`, given the engines' types.
pub fn bind(b: &mut Binder<'_>, types: &[EngineType]) -> Result<(), Error> {
    let piston = types.contains(&EngineType::Piston);
    let turbo = types
        .iter()
        .any(|t| matches!(t, EngineType::Turbine | EngineType::Turboprop));
    b.setter_only("propulsion/set-running", Kind::Int, |m, v| {
        m.exec.requests.push(ExecRequest::InitRunning(v as i32));
    })?;
    let starter = |b: &mut Binder<'_>| {
        b.property(
            "propulsion/starter_cmd",
            Kind::Int,
            |m| f64::from(m.propulsion.starter()),
            |m, v| m.propulsion.set_starter(v as i32),
        )
    };
    if turbo {
        starter(b)?;
        b.property(
            "propulsion/cutoff_cmd",
            Kind::Int,
            |m| f64::from(m.propulsion.cutoff()),
            |m, v| m.propulsion.set_cutoff(v as i32),
        )?;
    }
    if piston {
        starter(b)?;
        b.setter_only("propulsion/magneto_cmd", Kind::Int, |m, v| {
            m.propulsion.set_magnetos(v as i32);
        })?;
    }
    b.property(
        "propulsion/active_engine",
        Kind::Int,
        |m| f64::from(m.propulsion.active_engine),
        |m, v| m.propulsion.set_active_engine(v as i32),
    )?;
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("forces/fb{name}-prop-lbs"), move |m| {
            m.propulsion.forces[i]
        })?;
    }
    for (name, i) in [("l", E_X), ("m", E_Y), ("n", E_Z)] {
        b.getter(&format!("moments/{name}-prop-lbsft"), move |m| {
            m.propulsion.moments[i]
        })?;
    }
    b.property(
        "propulsion/total-fuel-lbs",
        Kind::Double,
        |m| m.propulsion.total_fuel_quantity,
        |m, v| m.propulsion.total_fuel_quantity = v,
    )?;
    b.property(
        "propulsion/total-oxidizer-lbs",
        Kind::Double,
        |m| m.propulsion.total_oxidizer_quantity,
        |m, v| m.propulsion.total_oxidizer_quantity = v,
    )?;
    b.property(
        "propulsion/refuel",
        Kind::Bool,
        |m| flag(m.propulsion.refuel),
        |m, v| m.propulsion.refuel = v != 0.0,
    )?;
    b.property(
        "propulsion/fuel_dump",
        Kind::Bool,
        |m| flag(m.propulsion.dump),
        |m, v| m.propulsion.dump = v != 0.0,
    )?;
    b.setter_only("propulsion/fuel_freeze", Kind::Bool, |m, v| {
        m.propulsion.set_fuel_freeze(v != 0.0);
    })?;
    Ok(())
}

fn engine(m: &Models, n: usize) -> Option<&super::Engine> {
    m.propulsion.engines.get(n)
}

/// The ties of `FGEngine::Load` for engine `n`.
pub fn bind_engine(b: &mut Binder<'_>, n: usize) -> Result<(), Error> {
    let base = format!("propulsion/engine[{n}]");
    b.property(
        &format!("{base}/set-running"),
        Kind::Bool,
        move |m| engine(m, n).map_or(0.0, |e| flag(e.base.running)),
        move |m, v| {
            if let Some(e) = m.propulsion.engines.get_mut(n) {
                e.base.running = v != 0.0;
            }
        },
    )?;
    b.getter(&format!("{base}/thrust-lbs"), move |m| {
        engine(m, n).map_or(0.0, |e| e.thruster.base.thrust)
    })?;
    b.getter(&format!("{base}/fuel-flow-rate-pps"), move |m| {
        engine(m, n).map_or(0.0, |e| e.base.fuel_flow_rate)
    })?;
    b.getter(&format!("{base}/fuel-flow-rate-gph"), move |m| {
        engine(m, n).map_or(0.0, |e| e.base.fuel_flow_rate_gph())
    })?;
    b.getter(&format!("{base}/fuel-used-lbs"), move |m| {
        engine(m, n).map_or(0.0, |e| e.base.fuel_used_lbs)
    })?;
    Ok(())
}
