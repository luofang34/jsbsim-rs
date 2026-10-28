// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGPropagate.cpp (bind).
//
//! Properties of `FGPropagate`, in JSBSim's binding order.

use crate::Error;
use crate::fdm::{Binder, Kind, Models};
use crate::math::{
    Axis, E_DOWN, E_EAST, E_NORTH, E_P, E_PHI, E_PSI, E_Q, E_R, E_THT, E_U, E_V, E_W, E_X, E_Y, E_Z,
};
use crate::units::{DEGTORAD, FTTOM};

fn velocities(b: &mut Binder<'_>) -> Result<(), Error> {
    b.getter("velocities/h-dot-fps", |m| m.propagate.hdot())?;
    for (name, i) in [("north", E_NORTH), ("east", E_EAST), ("down", E_DOWN)] {
        b.getter(&format!("velocities/v-{name}-fps"), move |m| {
            m.propagate.vel[i]
        })?;
    }
    for (name, i) in [("u", E_U), ("v", E_V), ("w", E_W)] {
        b.getter(&format!("velocities/{name}-fps"), move |m| {
            m.propagate.state.uvw[i]
        })?;
    }
    for (name, i) in [("p", E_P), ("q", E_Q), ("r", E_R)] {
        b.getter(&format!("velocities/{name}-rad_sec"), move |m| {
            m.propagate.state.pqr[i]
        })?;
    }
    for (name, i) in [("pi", E_P), ("qi", E_Q), ("ri", E_R)] {
        b.getter(&format!("velocities/{name}-rad_sec"), move |m| {
            m.propagate.state.pqri[i]
        })?;
    }
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("velocities/eci-{name}-fps"), move |m| {
            m.propagate.state.inertial_velocity[i]
        })?;
    }
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("velocities/ecef-{name}-fps"), move |m| {
            m.propagate.ecef_velocity()[i]
        })?;
    }
    b.getter("velocities/eci-velocity-mag-fps", |m| {
        m.propagate.state.inertial_velocity.magnitude()
    })?;
    b.getter("velocities/ned-velocity-mag-fps", |m| {
        m.propagate.state.uvw.magnitude()
    })?;
    Ok(())
}

fn position(b: &mut Binder<'_>) -> Result<(), Error> {
    let d = Kind::Double;
    b.property(
        "position/h-sl-ft",
        d,
        |m| m.propagate.altitude_asl(),
        |m, v| m.propagate.set_altitude_asl(v, &m.inertial),
    )?;
    b.property(
        "position/h-sl-meters",
        d,
        |m| m.propagate.altitude_asl() * FTTOM,
        |m, v| m.propagate.set_altitude_asl(v / FTTOM, &m.inertial),
    )?;
    b.property(
        "position/lat-gc-rad",
        d,
        |m| m.propagate.state.location.latitude(),
        |m, v| {
            m.propagate.set_latitude(v, &m.inertial);
        },
    )?;
    b.property(
        "position/long-gc-rad",
        d,
        |m| m.propagate.state.location.longitude(),
        |m, v| {
            m.propagate.set_longitude(v, &m.inertial);
        },
    )?;
    b.property(
        "position/lat-gc-deg",
        d,
        |m| m.propagate.state.location.latitude_deg(),
        |m, v| {
            m.propagate.set_latitude(v * DEGTORAD, &m.inertial);
        },
    )?;
    b.property(
        "position/long-gc-deg",
        d,
        |m| m.propagate.state.location.longitude_deg(),
        |m, v| {
            m.propagate.set_longitude(v * DEGTORAD, &m.inertial);
        },
    )?;
    Ok(())
}

fn position_geodetic(b: &mut Binder<'_>) -> Result<(), Error> {
    let d = Kind::Double;
    b.getter("position/lat-geod-rad", |m| {
        m.propagate.state.location.geod_latitude_rad()
    })?;
    b.getter("position/lat-geod-deg", |m| {
        m.propagate.state.location.geod_latitude_deg()
    })?;
    b.getter("position/geod-alt-ft", |m| {
        m.propagate.state.location.geod_altitude()
    })?;
    b.property(
        "position/h-agl-ft",
        d,
        |m| m.propagate.distance_agl(&m.inertial),
        |m, v| {
            m.propagate.set_distance_agl(v, &m.inertial);
        },
    )?;
    b.getter("position/geod-alt-km", |m| {
        m.propagate.state.location.geod_altitude() * 0.0003048
    })?;
    b.property(
        "position/h-agl-km",
        d,
        |m| m.propagate.distance_agl(&m.inertial) * 0.0003048,
        |m, v| m.propagate.set_distance_agl(v * 3280.8399, &m.inertial),
    )?;
    b.getter("position/radius-to-vehicle-ft", |m| m.propagate.radius())?;
    b.property(
        "position/terrain-elevation-asl-ft",
        d,
        |m| m.propagate.terrain_elevation(&m.inertial),
        |m, v| m.inertial.terrain_elevation = v,
    )?;
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("position/eci-{name}-ft"), move |m| {
            m.propagate.state.inertial_position[i]
        })?;
    }
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("position/ecef-{name}-ft"), move |m| {
            m.propagate.state.location.entry(i)
        })?;
    }
    b.getter("position/epa-rad", |m| m.propagate.epa)?;
    b.getter("metrics/terrain-radius", |m| {
        m.propagate.local_terrain_radius(&m.inertial)
    })?;
    Ok(())
}

fn attitude_and_orbit(b: &mut Binder<'_>) -> Result<(), Error> {
    let euler = |m: &Models, i: Axis| m.propagate.state.q_attitude_local.euler()[i];
    for (name, i) in [("phi", E_PHI), ("theta", E_THT), ("psi", E_PSI)] {
        b.getter(&format!("attitude/{name}-rad"), move |m| euler(m, i))?;
    }
    for (name, i) in [("phi", E_PHI), ("theta", E_THT), ("psi", E_PSI)] {
        b.getter(&format!("attitude/{name}-deg"), move |m| {
            m.propagate.euler_deg(i)
        })?;
    }
    for (name, i) in [("roll", E_PHI), ("pitch", E_THT), ("heading-true", E_PSI)] {
        b.getter(&format!("attitude/{name}-rad"), move |m| euler(m, i))?;
    }
    let d = Kind::Double;
    macro_rules! orbital {
        ($path:literal, $field:ident) => {
            b.property(
                $path,
                d,
                |m| m.propagate.orbit.$field,
                |m, v| m.propagate.orbit.$field = v,
            )?;
        };
    }
    orbital!("orbital/specific-angular-momentum-ft2_sec", h);
    orbital!("orbital/inclination-deg", inclination);
    orbital!("orbital/right-ascension-deg", right_ascension);
    orbital!("orbital/eccentricity", eccentricity);
    orbital!("orbital/argument-of-perigee-deg", perigee_argument);
    orbital!("orbital/true-anomaly-deg", true_anomaly);
    orbital!("orbital/apoapsis-radius-ft", apoapsis_radius);
    orbital!("orbital/periapsis-radius-ft", periapsis_radius);
    orbital!("orbital/period-sec", period);
    Ok(())
}

fn integrators(b: &mut Binder<'_>) -> Result<(), Error> {
    let i = Kind::Int;
    macro_rules! integrator {
        ($path:literal, $field:ident) => {
            b.property(
                $path,
                i,
                |m| f64::from(m.propagate.$field),
                |m, v| m.propagate.$field = v as i32,
            )?;
        };
    }
    integrator!(
        "simulation/integrator/rate/rotational",
        integrator_rotational_rate
    );
    integrator!(
        "simulation/integrator/rate/translational",
        integrator_translational_rate
    );
    integrator!(
        "simulation/integrator/position/rotational",
        integrator_rotational_position
    );
    integrator!(
        "simulation/integrator/position/translational",
        integrator_translational_position
    );
    b.setter_only("simulation/write-state-file", i, |_, v| {
        if v != 0.0 {
            tracing::error!("simulation/write-state-file: the core writes no files");
        }
    })?;
    Ok(())
}

/// `FGPropagate::bind`.
pub fn bind(b: &mut Binder<'_>) -> Result<(), Error> {
    velocities(b)?;
    position(b)?;
    position_geodetic(b)?;
    attitude_and_orbit(b)?;
    integrators(b)
}
