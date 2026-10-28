// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGAuxiliary.cpp (bind).
//
//! Properties of `FGAuxiliary`, in JSBSim's binding order.

use crate::Error;
use crate::fdm::{Binder, Models};
use crate::math::{E_PHI, E_PSI, E_THT, E_X, E_Y, E_Z};
use crate::units::{FPSTOKTS, RADTODEG};

type Get = fn(&Models) -> f64;

/// Total conditions and airspeeds, up to `velocities/machU`.
const AIRSPEEDS: [(&str, Get); 10] = [
    ("propulsion/tat-r", |m| m.auxiliary.airspeed.tat),
    ("propulsion/tat-c", |m| m.auxiliary.airspeed.tatc),
    ("propulsion/pt-lbs_sqft", |m| m.auxiliary.airspeed.pt),
    ("velocities/vc-fps", |m| m.auxiliary.airspeed.vcas),
    ("velocities/vc-kts", |m| {
        m.auxiliary.airspeed.vcas * FPSTOKTS
    }),
    ("velocities/ve-fps", |m| m.auxiliary.airspeed.veas),
    ("velocities/ve-kts", |m| {
        m.auxiliary.airspeed.veas * FPSTOKTS
    }),
    ("velocities/vtrue-fps", |m| m.auxiliary.vt),
    ("velocities/vtrue-kts", |m| m.auxiliary.vt * FPSTOKTS),
    ("velocities/machU", |m| m.auxiliary.airspeed.mach_u),
];

/// True airspeed, Mach and ground speed.
const MOTION: [(&str, Get); 3] = [
    ("velocities/vt-fps", |m| m.auxiliary.vt),
    ("velocities/mach", |m| m.auxiliary.airspeed.mach),
    ("velocities/vg-fps", |m| m.auxiliary.airspeed.vground),
];

/// Load factors.
const LOAD_FACTORS: [(&str, Get); 4] = [
    ("accelerations/Nx", |m| m.auxiliary.loads.nx),
    ("accelerations/Ny", |m| m.auxiliary.loads.ny),
    ("accelerations/Nz", |m| m.auxiliary.loads.nz),
    ("forces/load-factor", |m| m.auxiliary.nlf()),
];

/// Aerodynamic angles, pressures, heights and the flight path.
const AERO: [(&str, Get); 19] = [
    ("aero/alpha-rad", |m| m.auxiliary.alpha),
    ("aero/beta-rad", |m| m.auxiliary.beta),
    ("aero/mag-beta-rad", |m| m.auxiliary.mag_beta()),
    ("aero/alpha-deg", |m| m.auxiliary.alpha * RADTODEG),
    ("aero/beta-deg", |m| m.auxiliary.beta * RADTODEG),
    ("aero/mag-beta-deg", |m| m.auxiliary.mag_beta() * RADTODEG),
    ("aero/Re", |m| m.auxiliary.airspeed.re),
    ("aero/qbar-psf", |m| m.auxiliary.qbar),
    ("aero/qbarUW-psf", |m| m.auxiliary.airspeed.qbar_uw),
    ("aero/qbarUV-psf", |m| m.auxiliary.airspeed.qbar_uv),
    ("aero/alphadot-rad_sec", |m| m.auxiliary.adot),
    ("aero/betadot-rad_sec", |m| m.auxiliary.bdot),
    ("aero/alphadot-deg_sec", |m| m.auxiliary.adot * RADTODEG),
    ("aero/betadot-deg_sec", |m| m.auxiliary.bdot * RADTODEG),
    ("aero/h_b-cg-ft", |m| m.auxiliary.hoverbcg),
    ("aero/h_b-mac-ft", |m| m.auxiliary.hoverbmac),
    ("flight-path/gamma-rad", |m| m.auxiliary.gamma),
    ("flight-path/gamma-deg", |m| m.auxiliary.gamma * RADTODEG),
    ("flight-path/psi-gt-rad", |m| m.auxiliary.psigt),
];

/// Positions relative to the start and the VRP.
const POSITION: [(&str, Get); 6] = [
    ("position/distance-from-start-lon-mt", |m| {
        m.auxiliary
            .longitude_relative_position(m.ic.longitude_rad())
    }),
    ("position/distance-from-start-lat-mt", |m| {
        m.auxiliary
            .latitude_relative_position(m.ic.geod_latitude_rad())
    }),
    ("position/distance-from-start-mag-mt", |m| {
        m.auxiliary
            .distance_relative_position(m.ic.longitude_rad(), m.ic.geod_latitude_rad())
    }),
    ("position/vrp-gc-latitude_deg", |m| {
        m.auxiliary.location_vrp.latitude_deg()
    }),
    ("position/vrp-longitude_deg", |m| {
        m.auxiliary.location_vrp.longitude_deg()
    }),
    ("position/vrp-radius-ft", |m| {
        m.auxiliary.location_vrp.radius()
    }),
];

fn getters(b: &mut Binder<'_>, table: &[(&str, Get)]) -> Result<(), Error> {
    for &(path, get) in table {
        b.getter(path, get)?;
    }
    Ok(())
}

/// `FGAuxiliary::bind`.
pub fn bind(b: &mut Binder<'_>) -> Result<(), Error> {
    getters(b, &AIRSPEEDS)?;
    for (name, i) in [("p", E_X), ("q", E_Y), ("r", E_Z)] {
        b.getter(&format!("velocities/{name}-aero-rad_sec"), move |m| {
            m.auxiliary.aero_pqr[i]
        })?;
    }
    for (name, i) in [("phi", E_PHI), ("theta", E_THT), ("psi", E_PSI)] {
        b.getter(&format!("velocities/{name}dot-rad_sec"), move |m| {
            m.auxiliary.euler_rates[i]
        })?;
    }
    for (name, i) in [("u", E_X), ("v", E_Y), ("w", E_Z)] {
        b.getter(&format!("velocities/{name}-aero-fps"), move |m| {
            m.auxiliary.aero_uvw[i]
        })?;
    }
    getters(b, &MOTION)?;
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("accelerations/a-pilot-{name}-ft_sec2"), move |m| {
            m.auxiliary.loads.pilot_accel[i]
        })?;
    }
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("accelerations/n-pilot-{name}-norm"), move |m| {
            m.auxiliary.loads.pilot_accel_n[i]
        })?;
    }
    getters(b, &LOAD_FACTORS)?;
    getters(b, &AERO)?;
    getters(b, &POSITION)?;
    for (name, i) in [("n", E_X), ("e", E_Y), ("u", E_Z)] {
        b.getter(&format!("position/from-start-neu-{name}-ft"), move |m| {
            m.auxiliary.neu_position_from_start()[i]
        })?;
    }
    Ok(())
}
