// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGAuxiliary.h (Inputs).
//
//! What other models give the auxiliary model.

use crate::math::{ColumnVector3, Location, Matrix33};

/// The atmosphere at the vehicle.
#[derive(Debug, Clone, Copy, Default)]
pub struct Air {
    /// Static pressure, psf.
    pub pressure: f64,
    /// Density, slug/ft^3.
    pub density: f64,
    /// Static temperature, °R.
    pub temperature: f64,
    /// Speed of sound, ft/s.
    pub sound_speed: f64,
    /// Kinematic viscosity, ft^2/s.
    pub kinematic_viscosity: f64,
}

/// Values the executive sets once: `LoadPlanetConstants` and
/// `LoadModelConstants`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Constants {
    /// Standard-day sea-level speed of sound, ft/s.
    pub std_day_sl_soundspeed: f64,
    /// Standard gravity, ft/s^2.
    pub standard_gravity: f64,
    /// Wing span, ft.
    pub wingspan: f64,
    /// Mean aerodynamic chord, ft.
    pub wingchord: f64,
}

/// `FGAuxiliary::Inputs`.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// The atmosphere.
    pub air: Air,
    /// Planet and aircraft constants.
    pub constants: Constants,
    /// Height above the terrain, ft.
    pub distance_agl: f64,
    /// Mass, slugs.
    pub mass: f64,
    /// Local (NED) to body.
    pub tl2b: Matrix33,
    /// Body to local (NED).
    pub tb2l: Matrix33,
    /// Body rates relative to the ECEF frame, rad/s.
    pub pqr: ColumnVector3,
    /// Body rates relative to the ECI frame, rad/s.
    pub pqri: ColumnVector3,
    /// Their derivative, rad/s^2.
    pub pqridot: ColumnVector3,
    /// Body velocity, ft/s.
    pub uvw: ColumnVector3,
    /// Its derivative, ft/s^2.
    pub uvwdot: ColumnVector3,
    /// Local (NED) velocity, ft/s.
    pub vel: ColumnVector3,
    /// Body acceleration without gravity, ft/s^2.
    pub body_accel: ColumnVector3,
    /// Eye point, body frame, ft.
    pub to_eye_pt: ColumnVector3,
    /// Aerodynamic reference point, body frame, ft.
    pub rp_body: ColumnVector3,
    /// Visual reference point, body frame, ft.
    pub vrp_body: ColumnVector3,
    /// Aerodynamic forces, wind axes, lbs.
    pub fw: ColumnVector3,
    /// Vehicle position.
    pub location: Location,
    /// cos(theta).
    pub cos_tht: f64,
    /// sin(theta).
    pub sin_tht: f64,
    /// cos(phi).
    pub cos_phi: f64,
    /// sin(phi).
    pub sin_phi: f64,
    /// Steady wind plus turbulence and gusts, NED, ft/s.
    pub total_wind_ned: ColumnVector3,
    /// Turbulence rates, rad/s.
    pub turb_pqr: ColumnVector3,
}

crate::snapshot::visit_fields! {
    Air { pressure, density, temperature, sound_speed, kinematic_viscosity }
}
crate::snapshot::visit_fields! {
    Constants { std_day_sl_soundspeed, standard_gravity, wingspan, wingchord }
}
crate::snapshot::visit_fields! {
    Inputs {
        air, constants, distance_agl, mass, tl2b, tb2l, pqr, pqri, pqridot, uvw, uvwdot, vel,
        body_accel, to_eye_pt, rp_body, vrp_body, fw, location, cos_tht, sin_tht, cos_phi, sin_phi,
        total_wind_ned, turb_pqr,
    }
}
