// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGInitialCondition.cpp (bind).
//
//! The `ic/` properties, tied in JSBSim's order.

use super::Environment;
use crate::Error;
use crate::fdm::{Binder, Kind, Models};
use crate::math::{E_P, E_PHI, E_PSI, E_Q, E_R, E_THT, E_U, E_V, E_W};

fn env(m: &Models) -> Environment<'_> {
    Environment::new(&m.atmosphere, &m.inertial)
}

/// A read-write `ic/` property whose setter needs the environment.
macro_rules! tie_env {
    ($b:expr, $path:literal, $get:expr, $set:ident) => {
        $b.property($path, Kind::Double, $get, |m, v| {
            m.ic.$set(Environment::new(&m.atmosphere, &m.inertial), v)
        })?
    };
}

/// A read-write `ic/` property whose setter takes only the value.
macro_rules! tie {
    ($b:expr, $path:literal, $get:expr, $set:expr) => {
        $b.property($path, Kind::Double, $get, $set)?
    };
}

/// `FGInitialCondition::bind`.
pub fn bind(b: &mut Binder<'_>) -> Result<(), Error> {
    bind_speeds(b)?;
    bind_angles_and_position(b)?;
    bind_velocities(b)?;
    bind_rates_and_geodetic(b)
}

fn bind_speeds(b: &mut Binder<'_>) -> Result<(), Error> {
    tie_env!(
        b,
        "ic/vc-kts",
        |m| m.ic.vcalibrated_kts(env(m)),
        set_vcalibrated_kts
    );
    tie_env!(
        b,
        "ic/ve-kts",
        |m| m.ic.vequivalent_kts(env(m)),
        set_vequivalent_kts
    );
    tie!(b, "ic/vg-kts", |m| m.ic.vground_kts(), |m, v| m
        .ic
        .set_vground_kts(v));
    tie!(b, "ic/vt-kts", |m| m.ic.vtrue_kts(), |m, v| m
        .ic
        .set_vtrue_kts(v));
    tie_env!(b, "ic/mach", |m| m.ic.mach(env(m)), set_mach);
    tie!(b, "ic/roc-fpm", |m| m.ic.climb_rate_fpm(), |m, v| m
        .ic
        .set_climb_rate_fpm(v));
    tie!(
        b,
        "ic/gamma-deg",
        |m| m.ic.flight_path_angle_deg(),
        |m, v| m.ic.set_flight_path_angle_deg(v)
    );
    tie!(b, "ic/alpha-deg", |m| m.ic.alpha_deg(), |m, v| m
        .ic
        .set_alpha_deg(v));
    tie!(b, "ic/beta-deg", |m| m.ic.beta_deg(), |m, v| m
        .ic
        .set_beta_deg(v));
    Ok(())
}

fn bind_angles_and_position(b: &mut Binder<'_>) -> Result<(), Error> {
    tie!(b, "ic/theta-deg", |m| m.ic.euler_deg(E_THT), |m, v| m
        .ic
        .set_euler_angle_deg(E_THT, v));
    tie!(b, "ic/phi-deg", |m| m.ic.euler_deg(E_PHI), |m, v| m
        .ic
        .set_euler_angle_deg(E_PHI, v));
    tie!(b, "ic/psi-true-deg", |m| m.ic.euler_deg(E_PSI), |m, v| m
        .ic
        .set_euler_angle_deg(E_PSI, v));
    tie_env!(
        b,
        "ic/lat-gc-deg",
        |m| m.ic.latitude_deg(),
        set_latitude_deg
    );
    tie_env!(
        b,
        "ic/long-gc-deg",
        |m| m.ic.longitude_deg(),
        set_longitude_deg
    );
    tie_env!(
        b,
        "ic/h-sl-ft",
        |m| m.ic.altitude_asl_ft(),
        set_altitude_asl_ft
    );
    tie_env!(
        b,
        "ic/h-agl-ft",
        |m| m.ic.altitude_agl_ft(env(m)),
        set_altitude_agl_ft
    );
    b.property(
        "ic/terrain-elevation-ft",
        Kind::Double,
        |m| m.ic.terrain_elevation_ft(env(m)),
        |m, v| {
            m.ic.set_terrain_elevation_ft(&m.atmosphere, &mut m.inertial, v)
        },
    )?;
    Ok(())
}

fn bind_velocities(b: &mut Binder<'_>) -> Result<(), Error> {
    tie!(b, "ic/vg-fps", |m| m.ic.vground_fps(), |m, v| m
        .ic
        .set_vground_fps(v));
    tie!(b, "ic/vt-fps", |m| m.ic.vtrue_fps(), |m, v| m
        .ic
        .set_vtrue_fps(v));
    b.getter("ic/vw-bx-fps", |m| m.ic.body_wind_fps(E_U))?;
    b.getter("ic/vw-by-fps", |m| m.ic.body_wind_fps(E_V))?;
    b.getter("ic/vw-bz-fps", |m| m.ic.body_wind_fps(E_W))?;
    b.getter("ic/vw-north-fps", |m| m.ic.wind_ned_fps()[E_U])?;
    b.getter("ic/vw-east-fps", |m| m.ic.wind_ned_fps()[E_V])?;
    b.getter("ic/vw-down-fps", |m| m.ic.wind_ned_fps()[E_W])?;
    tie!(b, "ic/vw-mag-fps", |m| m.ic.wind_mag_fps(), |m, v| m
        .ic
        .set_wind_mag_fps(v));
    tie!(b, "ic/vw-dir-deg", |m| m.ic.wind_dir_deg(), |m, v| m
        .ic
        .set_wind_dir_deg(v));
    tie!(b, "ic/roc-fps", |m| m.ic.climb_rate_fps(), |m, v| m
        .ic
        .set_climb_rate_fps(v));
    for (path, idx) in [("ic/u-fps", E_U), ("ic/v-fps", E_V), ("ic/w-fps", E_W)] {
        b.property(
            path,
            Kind::Double,
            move |m| m.ic.body_vel_fps(idx),
            move |m, v| m.ic.set_body_vel_fps(idx, v),
        )?;
    }
    for (path, idx) in [("ic/vn-fps", E_U), ("ic/ve-fps", E_V), ("ic/vd-fps", E_W)] {
        b.property(
            path,
            Kind::Double,
            move |m| m.ic.ned_vel_fps(idx),
            move |m, v| m.ic.set_ned_vel_fps(idx, v),
        )?;
    }
    Ok(())
}

fn bind_rates_and_geodetic(b: &mut Binder<'_>) -> Result<(), Error> {
    tie!(
        b,
        "ic/gamma-rad",
        |m| m.ic.flight_path_angle_rad(),
        |m, v| m.ic.set_flight_path_angle_rad(v)
    );
    tie!(b, "ic/alpha-rad", |m| m.ic.alpha_rad(), |m, v| m
        .ic
        .set_alpha_rad(v));
    tie!(b, "ic/theta-rad", |m| m.ic.euler_rad(E_THT), |m, v| m
        .ic
        .set_euler_angle_rad(E_THT, v));
    tie!(b, "ic/beta-rad", |m| m.ic.beta_rad(), |m, v| m
        .ic
        .set_beta_rad(v));
    tie!(b, "ic/phi-rad", |m| m.ic.euler_rad(E_PHI), |m, v| m
        .ic
        .set_euler_angle_rad(E_PHI, v));
    tie!(b, "ic/psi-true-rad", |m| m.ic.euler_rad(E_PSI), |m, v| m
        .ic
        .set_euler_angle_rad(E_PSI, v));
    tie_env!(
        b,
        "ic/lat-gc-rad",
        |m| m.ic.latitude_rad(),
        set_latitude_rad
    );
    tie_env!(
        b,
        "ic/long-gc-rad",
        |m| m.ic.longitude_rad(),
        set_longitude_rad
    );
    for (path, idx) in [
        ("ic/p-rad_sec", E_P),
        ("ic/q-rad_sec", E_Q),
        ("ic/r-rad_sec", E_R),
    ] {
        b.property(
            path,
            Kind::Double,
            move |m| m.ic.pqr_radps()[idx],
            move |m, v| m.ic.set_pqr_radps(idx, v),
        )?;
    }
    tie_env!(
        b,
        "ic/lat-geod-rad",
        |m| m.ic.geod_latitude_rad(),
        set_geod_latitude_rad
    );
    tie_env!(
        b,
        "ic/lat-geod-deg",
        |m| m.ic.geod_latitude_deg(),
        set_geod_latitude_deg
    );
    b.getter("ic/geod-alt-ft", |m| m.ic.position().geod_altitude())?;
    tie!(b, "ic/targetNlf", |m| m.ic.target_nlf(), |m, v| m
        .ic
        .set_target_nlf(v));
    Ok(())
}
