// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGInitialCondition.cpp (Load,
// Load_v1, LoadLatitude, SetTrimRequest).
//
//! Initialization files: the `<initialize>` document of format 1 (flat
//! elements applied through the setters) or 2 (frames, see `load_v2`).
//!
//! Trimming is not ported: a version 1 `<trim>` that requests a trim is
//! refused before anything is applied. The `<running>` engines are
//! recorded for `RunIC` to start.

use std::f64::consts::PI;

use super::{Environment, InitialCondition, LatitudeSet};
use crate::Error;
use crate::math::{E_PHI, E_PSI, E_THT, E_U, E_V, E_W, Quaternion};
use crate::models::atmosphere::Atmosphere;
use crate::models::inertial::Inertial;
use crate::xml::Element;

/// What `Load` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoadOutcome {
    /// `Load`'s return value: false when the file is inconsistent (a
    /// latitude out of range, a missing position, an unknown frame, a
    /// version below 1); the problem has been logged.
    pub result: bool,
    /// Whether `<planet_rotation_rate>` changed the planet's rotation,
    /// which the propagation and acceleration inputs then take.
    pub planet_rotation_changed: bool,
}

fn model_error(message: String) -> Error {
    Error::Model { message }
}

/// `SetTrimRequest`: the trim mode `value` asks for; anything unknown,
/// including `0`, asks for none.
fn trim_mode(value: &str) -> Option<&'static str> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "ground" => Some("ground"),
        "longitudinal" => Some("longitudinal"),
        "full" => Some("full"),
        "pullup" => Some("pullup"),
        "custom" => Some("custom"),
        "turn" => Some("turn"),
        _ => None,
    }
}

impl InitialCondition {
    /// `Load(document)`: applies the `<initialize>` document on top of the
    /// current state, which is not reset first.
    pub fn load(
        &mut self,
        document: &Element,
        atmosphere: &Atmosphere,
        inertial: &mut Inertial,
    ) -> Result<LoadOutcome, Error> {
        if document.name() != "initialize" {
            return Err(model_error(format!(
                "File: {} is not a reset file.",
                document.file_name()
            )));
        }
        let version = if document.has_attribute("version") {
            Some(document.attribute_as_number("version")?)
        } else {
            None
        };
        if version.is_some_and(|v| v >= 3.0) {
            return Err(model_error(format!(
                "{}: Only initialization file formats 1 and 2 are currently supported",
                document.location()
            )));
        }
        let mut outcome = LoadOutcome::default();
        match version {
            Some(v) if v >= 2.0 => outcome = self.load_v2(document, atmosphere, inertial)?,
            Some(v) if v < 1.0 => {}
            _ => outcome.result = self.load_v1(document, atmosphere, inertial)?,
        }
        // Engine -1 is every engine.
        for running in document.find_elements("running") {
            let n = running.data_as_number()? as i32;
            self.engines_running |= match n {
                -1 => u32::MAX,
                n => u32::try_from(n)
                    .ok()
                    .and_then(|n| 1u32.checked_shl(n))
                    .unwrap_or(0),
            };
        }
        Ok(outcome)
    }

    fn refuse_trim(document: &Element) -> Result<(), Error> {
        let Some(trim) = document.find_element("trim") else {
            return Ok(());
        };
        let value = document.find_element_value("trim");
        match trim_mode(value) {
            Some(mode) => Err(Error::Unsupported {
                construct: format!("<trim> {mode} (FGTrim)"),
                path: trim.location().to_string(),
            }),
            None => Ok(()),
        }
    }

    /// `Load_v1`: position, then orientation (set directly, so the local
    /// velocity is kept), then speeds, angles and wind through the setters.
    fn load_v1(
        &mut self,
        document: &Element,
        atmosphere: &Atmosphere,
        inertial: &mut Inertial,
    ) -> Result<bool, Error> {
        Self::refuse_trim(document)?;
        let value = |name: &str, unit: &str| -> Result<Option<f64>, Error> {
            match document.find_element(name) {
                Some(_) => Ok(Some(
                    document.find_element_value_as_number_convert_to(name, unit)?,
                )),
                None => Ok(None),
            }
        };
        if let Some(v) = value("longitude", "RAD")? {
            self.set_longitude_rad(Environment::new(atmosphere, inertial), v);
        }
        if let Some(v) = value("elevation", "FT")? {
            self.set_terrain_elevation_ft(atmosphere, inertial, v);
        }
        let env = Environment::new(atmosphere, inertial);
        if let Some(v) = value("altitude", "FT")? {
            self.set_altitude_agl_ft(env, v);
        } else if let Some(v) = value("altitudeAGL", "FT")? {
            self.set_altitude_agl_ft(env, v);
        } else if let Some(v) = value("altitudeMSL", "FT")? {
            self.set_altitude_asl_ft(env, v);
        }
        let result = self.load_latitude(document, env)?;

        let mut orient = self.orientation.euler();
        for (name, idx) in [("phi", E_PHI), ("theta", E_THT), ("psi", E_PSI)] {
            if let Some(v) = value(name, "RAD")? {
                orient[idx] = v;
            }
        }
        self.orientation = Quaternion::from_euler_vector(orient);

        for (name, idx) in [("ubody", E_U), ("vbody", E_V), ("wbody", E_W)] {
            if let Some(v) = value(name, "FT/SEC")? {
                self.set_body_vel_fps(idx, v);
            }
        }
        for (name, idx) in [("vnorth", E_U), ("veast", E_V), ("vdown", E_W)] {
            if let Some(v) = value(name, "FT/SEC")? {
                self.set_ned_vel_fps(idx, v);
            }
        }
        self.load_v1_speeds_and_wind(document, env)?;
        self.pqr_body = crate::math::ColumnVector3::zero();
        Ok(result)
    }

    fn load_v1_speeds_and_wind(
        &mut self,
        document: &Element,
        env: Environment<'_>,
    ) -> Result<(), Error> {
        let value = |name: &str, unit: &str| -> Result<Option<f64>, Error> {
            match document.find_element(name) {
                Some(_) => Ok(Some(
                    document.find_element_value_as_number_convert_to(name, unit)?,
                )),
                None => Ok(None),
            }
        };
        if let Some(v) = value("vc", "KTS")? {
            self.set_vcalibrated_kts(env, v);
        }
        if let Some(v) = value("vt", "KTS")? {
            self.set_vtrue_kts(v);
        }
        if document.find_element("mach").is_some() {
            self.set_mach(env, document.find_element_value_as_number("mach")?);
        }
        if let Some(v) = value("gamma", "DEG")? {
            self.set_flight_path_angle_deg(v);
        }
        if let Some(v) = value("roc", "FT/SEC")? {
            self.set_climb_rate_fps(v);
        }
        if let Some(v) = value("vground", "KTS")? {
            self.set_vground_kts(v);
        }
        if let Some(v) = value("alpha", "DEG")? {
            self.set_alpha_deg(v);
        }
        if let Some(v) = value("beta", "DEG")? {
            self.set_beta_deg(v);
        }
        if let Some(v) = value("vwind", "KTS")? {
            self.set_wind_mag_kts(v);
        }
        if let Some(v) = value("winddir", "DEG")? {
            self.set_wind_dir_deg(v);
        }
        if let Some(v) = value("hwind", "KTS")? {
            self.set_head_wind_kts(v);
        }
        if let Some(v) = value("xwind", "KTS")? {
            self.set_cross_wind_kts(v);
        }
        if document.find_element("targetNlf").is_some() {
            self.set_target_nlf(document.find_element_value_as_number("targetNlf")?);
        }
        Ok(())
    }

    /// `LoadLatitude`: false, logged, when the latitude is outside
    /// [-pi/2, pi/2].
    pub(super) fn load_latitude(
        &mut self,
        position_el: &Element,
        env: Environment<'_>,
    ) -> Result<bool, Error> {
        let Some(latitude_el) = position_el.find_element("latitude") else {
            return Ok(true);
        };
        let latitude = position_el.find_element_value_as_number_convert_to("latitude", "RAD")?;
        if latitude.abs() > 0.5 * PI {
            let unit = match latitude_el.attribute("unit") {
                "" => "RAD",
                u => u,
            };
            let range = if unit == "DEG" {
                "-90 DEG ; +90 DEG]"
            } else {
                "-PI/2 RAD; +PI/2 RAD]"
            };
            tracing::error!(
                "{}: The latitude value {} {unit} is outside the range [{range}",
                latitude_el.location(),
                latitude_el.data_line(0)
            );
            return Ok(false);
        }
        match latitude_el.attribute("type") {
            "geod" | "geodetic" => {
                self.set_geod_latitude_rad(env, latitude);
                self.last_latitude_set = LatitudeSet::Geodetic;
            }
            _ => {
                self.set_latitude_rad(env, latitude);
                self.last_latitude_set = LatitudeSet::Geocentric;
            }
        }
        Ok(true)
    }
}
