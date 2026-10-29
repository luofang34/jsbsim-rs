// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/atmosphere/FGWinds.cpp (bind).
//
//! Properties of `FGWinds`, in JSBSim's binding order.

use crate::Error;
use crate::fdm::{Binder, Kind};
use crate::math::{Axis, E_DOWN, E_EAST, E_NORTH, E_P, E_Q, E_R, E_X, E_Y, E_Z};

const NED: [(&str, Axis); 3] = [("north", E_NORTH), ("east", E_EAST), ("down", E_DOWN)];

/// `FGWinds::bind`.
pub fn bind(b: &mut Binder<'_>) -> Result<(), Error> {
    bind_wind(b)?;
    bind_cosine_gust(b)?;
    b.setter_only(
        "atmosphere/updownburst/number-of-cells",
        Kind::Int,
        |m, v| {
            m.winds.set_updownburst_cells(v as i32);
        },
    )?;
    bind_turbulence(b)?;
    for (name, i) in NED {
        b.getter(&format!("atmosphere/total-wind-{name}-fps"), move |m| {
            m.winds.total_wind_ned[i]
        })?;
    }
    b.property(
        "atmosphere/randomseed",
        Kind::Int,
        |m| f64::from(m.winds.random_seed(m.exec.random_seed)),
        |m, v| m.winds.set_random_seed(v as i32),
    )?;
    Ok(())
}

/// The steady wind and the user gust.
fn bind_wind(b: &mut Binder<'_>) -> Result<(), Error> {
    let d = Kind::Double;
    b.property(
        "atmosphere/psiw-rad",
        d,
        |m| m.winds.psiw,
        |m, v| m.winds.set_wind_psi(v),
    )?;
    for (name, i) in NED {
        b.property(
            &format!("atmosphere/wind-{name}-fps"),
            d,
            move |m| m.winds.wind_ned[i],
            move |m, v| m.winds.set_wind_ned_component(i, v),
        )?;
    }
    b.property(
        "atmosphere/wind-mag-fps",
        d,
        |m| m.winds.windspeed(),
        |m, v| m.winds.set_windspeed(v),
    )?;
    for (name, i) in NED {
        b.property(
            &format!("atmosphere/gust-{name}-fps"),
            d,
            move |m| m.winds.gust_ned[i],
            move |m, v| m.winds.gust_ned[i] = v,
        )?;
    }
    Ok(())
}

/// The one-minus-cosine gust's write-only parameters.
fn bind_cosine_gust(b: &mut Binder<'_>) -> Result<(), Error> {
    let d = Kind::Double;
    let p = "atmosphere/cosine-gust/";
    b.setter_only(&format!("{p}startup-duration-sec"), d, |m, v| {
        m.winds.cosine_gust.profile.startup_duration = v;
    })?;
    b.setter_only(&format!("{p}steady-duration-sec"), d, |m, v| {
        m.winds.cosine_gust.profile.steady_duration = v;
    })?;
    b.setter_only(&format!("{p}end-duration-sec"), d, |m, v| {
        m.winds.cosine_gust.profile.end_duration = v;
    })?;
    b.setter_only(&format!("{p}magnitude-ft_sec"), d, |m, v| {
        m.winds.cosine_gust.magnitude = v;
    })?;
    b.setter_only(&format!("{p}frame"), Kind::Int, |m, v| {
        m.winds.cosine_gust.frame = v as i32;
    })?;
    for (name, i) in [("X", E_X), ("Y", E_Y), ("Z", E_Z)] {
        b.setter_only(&format!("{p}{name}-velocity-ft_sec"), d, move |m, v| {
            m.winds.cosine_gust.direction[i] = v;
        })?;
    }
    b.setter_only(&format!("{p}start"), Kind::Bool, |m, v| {
        m.winds.cosine_gust.profile.running = v != 0.0;
    })?;
    Ok(())
}

/// The turbulence state and parameters.
fn bind_turbulence(b: &mut Binder<'_>) -> Result<(), Error> {
    let d = Kind::Double;
    for (name, i) in NED {
        b.property(
            &format!("atmosphere/turb-{name}-fps"),
            d,
            move |m| m.winds.turbulence_ned[i],
            move |m, v| m.winds.turbulence_ned[i] = v,
        )?;
    }
    for (name, i) in [("p", E_P), ("q", E_Q), ("r", E_R)] {
        b.getter(&format!("atmosphere/{name}-turb-rad_sec"), move |m| {
            m.winds.turb_pqr[i]
        })?;
    }
    b.property(
        "atmosphere/turb-type",
        Kind::Int,
        |m| f64::from(m.winds.turbulence.turb_type),
        |m, v| m.winds.turbulence.turb_type = v as i32,
    )?;
    b.property(
        "atmosphere/turb-rate",
        d,
        |m| m.winds.turbulence.rate,
        |m, v| m.winds.turbulence.rate = v,
    )?;
    b.property(
        "atmosphere/turb-gain",
        d,
        |m| m.winds.turbulence.gain,
        |m, v| m.winds.turbulence.gain = v,
    )?;
    b.property(
        "atmosphere/turb-rhythmicity",
        d,
        |m| m.winds.turbulence.rhythmicity,
        |m, v| m.winds.turbulence.rhythmicity = v,
    )?;
    b.property(
        "atmosphere/turbulence/milspec/windspeed_at_20ft_AGL-fps",
        d,
        |m| m.winds.turbulence.windspeed_at_20ft,
        |m, v| m.winds.turbulence.windspeed_at_20ft = v,
    )?;
    b.property(
        "atmosphere/turbulence/milspec/severity",
        Kind::Int,
        |m| f64::from(m.winds.turbulence.severity),
        |m, v| m.winds.turbulence.severity = v as i32,
    )?;
    Ok(())
}
