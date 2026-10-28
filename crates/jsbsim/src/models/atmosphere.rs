// SPDX-License-Identifier: LGPL-2.1-or-later
//! The atmosphere (`FGAtmosphere` / `FGStandardAtmosphere`). Placeholder:
//! the 1976 standard atmosphere is being ported.

use super::base::ModelBase;

/// Inputs loaded by the executive.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inputs {
    /// Altitude above sea level, ft.
    pub altitude_asl: f64,
    /// Geodetic latitude, deg.
    pub geod_latitude_deg: f64,
    /// Longitude, deg.
    pub longitude_deg: f64,
}

/// The atmosphere.
#[derive(Debug, Clone, Default)]
pub struct Atmosphere {
    /// Scheduling.
    pub base: ModelBase,
    /// Inputs of the current frame.
    pub input: Inputs,
    /// Pressure, psf.
    pub pressure: f64,
    /// Density, slug/ft^3.
    pub density: f64,
    /// Temperature, °R.
    pub temperature: f64,
    /// Speed of sound, ft/s.
    pub sound_speed: f64,
    /// Kinematic viscosity, ft^2/s.
    pub kinematic_viscosity: f64,
    /// Pressure / sea-level pressure.
    pub pressure_ratio: f64,
    /// Density / sea-level density.
    pub density_ratio: f64,
    /// Standard-day sea-level speed of sound, ft/s.
    pub std_day_sl_soundspeed: f64,
}
