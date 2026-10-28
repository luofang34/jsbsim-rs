// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGAerodynamics.cpp (bind).
//
//! Properties of `FGAerodynamics`, in JSBSim's binding order.

use crate::Error;
use crate::fdm::{Binder, Kind, Models};
use crate::math::{
    ColumnVector3, E_DRAG, E_L, E_LIFT, E_M, E_N, E_PITCH, E_ROLL, E_SIDE, E_X, E_Y, E_YAW, E_Z,
};

fn stability_forces(m: &Models) -> ColumnVector3 {
    let s = &m.aerodynamics.state;
    let mut v = s.tb2s * s.forces;
    v[E_DRAG] *= -1.0;
    v[E_LIFT] *= -1.0;
    v
}

/// `FGAerodynamics::bind`.
pub fn bind(b: &mut Binder<'_>) -> Result<(), Error> {
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("forces/fb{name}-aero-lbs"), move |m| {
            m.aerodynamics.state.forces[i]
        })?;
    }
    for (name, i) in [("l", E_L), ("m", E_M), ("n", E_N)] {
        b.getter(&format!("moments/{name}-aero-lbsft"), move |m| {
            m.aerodynamics.state.moments[i]
        })?;
    }
    for (name, i) in [("x", E_DRAG), ("y", E_SIDE), ("z", E_LIFT)] {
        b.getter(&format!("forces/fw{name}-aero-lbs"), move |m| {
            m.aerodynamics.state.fw[i]
        })?;
    }
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("forces/fs{name}-aero-lbs"), move |m| {
            stability_forces(m)[i]
        })?;
    }
    for (name, i) in [("roll", E_ROLL), ("pitch", E_PITCH), ("yaw", E_YAW)] {
        b.getter(&format!("moments/{name}-stab-aero-lbsft"), move |m| {
            let s = &m.aerodynamics.state;
            (s.tb2s * s.moments)[i]
        })?;
    }
    for (name, i) in [("roll", E_ROLL), ("pitch", E_PITCH), ("yaw", E_YAW)] {
        b.getter(&format!("moments/{name}-wind-aero-lbsft"), move |m| {
            (m.aerodynamics.input.tb2w * m.aerodynamics.state.moments)[i]
        })?;
    }
    b.getter("forces/lod-norm", |m| m.aerodynamics.state.lod)?;
    b.getter("aero/cl-squared", |m| m.aerodynamics.state.clsq)?;
    b.property(
        "aero/qbar-area",
        Kind::Double,
        |m| m.aerodynamics.state.qbar_area,
        |m, v| {
            m.aerodynamics.state.qbar_area = v;
        },
    )?;
    b.property(
        "aero/alpha-max-rad",
        Kind::Double,
        |m| m.aerodynamics.state.alpha_cl_max,
        |m, v| {
            m.aerodynamics.state.alpha_cl_max = v;
        },
    )?;
    b.property(
        "aero/alpha-min-rad",
        Kind::Double,
        |m| m.aerodynamics.state.alpha_cl_min,
        |m, v| {
            m.aerodynamics.state.alpha_cl_min = v;
        },
    )?;
    b.getter("aero/bi2vel", |m| m.aerodynamics.state.bi2vel)?;
    b.getter("aero/ci2vel", |m| m.aerodynamics.state.ci2vel)?;
    b.getter("aero/alpha-wing-rad", |m| m.aerodynamics.state.alphaw)?;
    b.getter("systems/stall-warn-norm", |m| {
        m.aerodynamics.state.impending_stall
    })?;
    b.getter("aero/stall-hyst-norm", |m| m.aerodynamics.state.stall_hyst)?;
    Ok(())
}
