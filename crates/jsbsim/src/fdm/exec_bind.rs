// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.cpp (constructor ties) and
// src/models/FGModel.cpp (enable ties).
//
//! Properties of the executive and the model enable flags.

use super::{Binder, Kind, Models};
use crate::Error;
use crate::models::base::ModelBase;

/// Requests from setters that need the whole executive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecRequest {
    /// `simulation/reset` with its mode bits.
    Reset(i32),
    /// `simulation/do_simple_trim` with its mode.
    Trim(i32),
    /// `simulation/do_linearization`.
    Linearize,
    /// `propulsion/set-running` with the engine, -1 for all.
    InitRunning(i32),
}

fn flag(b: bool) -> f64 {
    f64::from(u8::from(b))
}

/// Ties `simulation/models/<name>/enabled` to the model base `get` and
/// `set` select.
pub fn bind_enable(
    b: &mut Binder<'_>,
    name: &str,
    get: fn(&Models) -> &ModelBase,
    set: fn(&mut Models) -> &mut ModelBase,
) -> Result<(), Error> {
    b.property(
        &format!("simulation/models/{name}/enabled"),
        Kind::Bool,
        move |m| flag(get(m).enabled),
        move |m, v| set(m).enabled = v != 0.0,
    )?;
    Ok(())
}

/// `simulation/models/FGAtmosphere/enabled`, which keeps reading and
/// writing the replaced model's flag after a planet replaces the
/// atmosphere.
pub fn bind_atmosphere_enable(b: &mut Binder<'_>) -> Result<(), Error> {
    b.property(
        "simulation/models/FGAtmosphere/enabled",
        Kind::Bool,
        |m| {
            let a = &m.atmosphere;
            flag(a.orphaned_enabled.unwrap_or(a.base.enabled))
        },
        |m, v| {
            let a = &mut m.atmosphere;
            match &mut a.orphaned_enabled {
                Some(orphan) => *orphan = v != 0.0,
                None => a.base.enabled = v != 0.0,
            }
        },
    )?;
    Ok(())
}

/// The executive's own properties, bound after the models.
pub fn bind_exec(b: &mut Binder<'_>) -> Result<(), Error> {
    b.setter_only("simulation/do_simple_trim", Kind::Int, |m, v| {
        m.exec.requests.push(ExecRequest::Trim(v as i32))
    })?;
    b.setter_only("simulation/do_linearization", Kind::Int, |m, _| {
        m.exec.requests.push(ExecRequest::Linearize)
    })?;
    b.setter_only("simulation/reset", Kind::Int, |m, v| {
        m.exec.requests.push(ExecRequest::Reset(v as i32))
    })?;
    b.typed_getter("simulation/disperse", Kind::Int, |_| 0.0)?;
    b.property(
        "simulation/randomseed",
        Kind::Int,
        |m| f64::from(m.exec.random_seed),
        // `SRand(int)`: `RandomSeed` is unsigned, and the shared
        // generator is reseeded in place.
        |m, v| {
            m.exec.random_seed = v as i32;
            m.exec.rng.seed(m.exec.random_seed as u32);
        },
    )?;
    b.property(
        "simulation/terminate",
        Kind::Bool,
        |m| flag(m.exec.terminate),
        |m, v| m.exec.terminate = v != 0.0,
    )?;
    b.property(
        "simulation/pause",
        Kind::Bool,
        |m| flag(m.exec.holding),
        |m, v| m.exec.holding = v != 0.0,
    )?;
    b.getter("simulation/sim-time-sec", |m| m.exec.sim_time)?;
    b.getter("simulation/dt", |m| m.exec.dt)?;
    b.property(
        "simulation/jsbsim-debug",
        Kind::Int,
        |m| f64::from(m.exec.debug_level),
        |m, v| {
            m.exec.debug_level = v as i32;
        },
    )?;
    b.property(
        "simulation/frame",
        Kind::Int,
        |m| f64::from(m.exec.frame as i32),
        |m, v| m.exec.frame = v as i32 as u32,
    )?;
    b.property(
        "simulation/trim-completed",
        Kind::Int,
        |m| f64::from(m.exec.trim_completed),
        |m, v| {
            m.exec.trim_completed = v as i32;
        },
    )?;
    b.property(
        "forces/hold-down",
        Kind::Bool,
        |m| flag(m.exec.hold_down),
        |m, v| m.set_hold_down(v != 0.0),
    )?;
    Ok(())
}
