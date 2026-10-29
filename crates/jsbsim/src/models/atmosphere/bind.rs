// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGAtmosphere.cpp and
// src/models/atmosphere/FGStandardAtmosphere.cpp (bind).
//
//! Properties of the atmosphere: `FGAtmosphere::bind` (run by the base
//! constructor) and then `FGStandardAtmosphere::bind`.

use super::{PressureUnit, TemperatureUnit};
use crate::Error;
use crate::fdm::{Binder, Kind, Models};

type Get = fn(&Models) -> f64;

/// `FGAtmosphere::bind`: read-only state and ratios.
const BASE_GETTERS: [(&str, Get); 13] = [
    ("atmosphere/T-R", |m| m.atmosphere.temperature),
    ("atmosphere/rho-slugs_ft3", |m| m.atmosphere.density),
    ("atmosphere/P-psf", |m| m.atmosphere.pressure),
    ("atmosphere/a-fps", |m| m.atmosphere.sound_speed),
    ("atmosphere/T-sl-R", |m| m.atmosphere.get_temperature_sl()),
    ("atmosphere/rho-sl-slugs_ft3", |m| {
        m.atmosphere.get_density_sl()
    }),
    ("atmosphere/a-sl-fps", |m| m.atmosphere.get_sound_speed_sl()),
    ("atmosphere/theta", |m| m.atmosphere.get_temperature_ratio()),
    ("atmosphere/sigma", |m| m.atmosphere.get_density_ratio()),
    ("atmosphere/delta", |m| m.atmosphere.get_pressure_ratio()),
    ("atmosphere/a-ratio", |m| {
        m.atmosphere.get_sound_speed_ratio()
    }),
    ("atmosphere/density-altitude", |m| {
        m.atmosphere.get_density_altitude()
    }),
    ("atmosphere/pressure-altitude", |m| {
        m.atmosphere.get_pressure_altitude()
    }),
];

/// The properties `FGStandardAtmosphere::bind` adds to `FGAtmosphere`'s,
/// which an atmosphere that is not a standard one leaves untied.
pub const STANDARD_PROPERTIES: [&str; 8] = [
    "atmosphere/delta-T",
    "atmosphere/SL-graded-delta-T",
    "atmosphere/P-sl-psf",
    "atmosphere/dew-point-R",
    "atmosphere/vapor-pressure-psf",
    "atmosphere/saturated-vapor-pressure-psf",
    "atmosphere/RH",
    "atmosphere/vapor-fraction-ppm",
];

/// Binds the atmosphere's properties in JSBSim's order.
pub fn bind(b: &mut Binder<'_>) -> Result<(), Error> {
    for (path, get) in BASE_GETTERS {
        b.getter(path, get)?;
    }
    bind_standard(b)
}

/// `FGStandardAtmosphere::bind`.
fn bind_standard(b: &mut Binder<'_>) -> Result<(), Error> {
    let d = Kind::Double;
    let (r, psf) = (TemperatureUnit::Rankine, PressureUnit::Psf);
    b.property(
        "atmosphere/delta-T",
        d,
        move |m| m.atmosphere.get_temperature_bias(r),
        move |m, v| m.atmosphere.set_temperature_bias(r, v),
    )?;
    b.property(
        "atmosphere/SL-graded-delta-T",
        d,
        move |m| m.atmosphere.get_temperature_delta_gradient(r),
        move |m, v| m.atmosphere.set_sl_temperature_graded_delta(r, v),
    )?;
    b.property(
        "atmosphere/P-sl-psf",
        d,
        move |m| m.atmosphere.get_pressure_sl(psf),
        move |m, v| m.atmosphere.set_pressure_sl(psf, v),
    )?;
    b.property(
        "atmosphere/dew-point-R",
        d,
        move |m| m.atmosphere.get_dew_point(r),
        move |m, v| m.atmosphere.set_dew_point(r, v),
    )?;
    b.property(
        "atmosphere/vapor-pressure-psf",
        d,
        move |m| m.atmosphere.get_vapor_pressure(psf),
        move |m, v| m.atmosphere.set_vapor_pressure(psf, v),
    )?;
    b.getter("atmosphere/saturated-vapor-pressure-psf", move |m| {
        m.atmosphere.get_saturated_vapor_pressure(psf)
    })?;
    b.property(
        "atmosphere/RH",
        d,
        |m| m.atmosphere.get_relative_humidity(),
        |m, v| m.atmosphere.set_relative_humidity(v),
    )?;
    b.property(
        "atmosphere/vapor-fraction-ppm",
        d,
        |m| m.atmosphere.get_vapor_mass_fraction_ppm(),
        |m, v| m.atmosphere.set_vapor_mass_fraction_ppm(v),
    )?;
    Ok(())
}
