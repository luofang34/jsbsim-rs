// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGInitialCondition.cpp (altitude,
// terrain, latitude and longitude setters and getters).
//
//! Position. Altitudes keep the latitude of the kind set last (geocentric
//! or geodetic); latitudes and longitudes keep the altitude of the kind set
//! last (above sea level or ground). A speed set as calibrated, equivalent
//! or Mach is kept when the altitude changes.

use super::airspeed::{mach_from_vcalibrated, vcalibrated_from_mach};
use super::{AltitudeSet, Environment, InitialCondition, LatitudeSet, SpeedSet};
use crate::models::atmosphere::STD_DAY_SL_DENSITY;
use crate::models::inertial::Inertial;
use crate::units::{DEGTORAD, sign};

/// Calibrated and equivalent airspeeds and Mach number to restore after an
/// altitude change.
struct HeldSpeed {
    mach: f64,
    vc: f64,
    ve: f64,
}

impl InitialCondition {
    fn held_speed(&self, env: Environment<'_>) -> HeldSpeed {
        let a = env.atmosphere;
        let altitude_asl = self.altitude_asl_ft();
        let pressure = a.get_pressure(altitude_asl);
        let sound_speed = a.get_sound_speed(altitude_asl);
        let rho = a.get_density(altitude_asl);
        let mach = self.vt / sound_speed;
        HeldSpeed {
            mach,
            vc: vcalibrated_from_mach(mach, pressure, a.std_day_sl_soundspeed),
            ve: self.vt * (rho / STD_DAY_SL_DENSITY).sqrt(),
        }
    }

    /// Restores the speed set last at the atmosphere of `altitude_asl`.
    fn restore_speed(&mut self, env: Environment<'_>, held: &HeldSpeed, altitude_asl: f64) {
        let a = env.atmosphere;
        let sound_speed = a.get_sound_speed(altitude_asl);
        let rho = a.get_density(altitude_asl);
        let pressure = a.get_pressure(altitude_asl);
        match self.last_speed_set {
            SpeedSet::Vc => {
                let mach = mach_from_vcalibrated(held.vc, pressure, a.std_day_sl_soundspeed);
                self.set_vtrue_fps(mach * sound_speed);
            }
            SpeedSet::Mach => self.set_vtrue_fps(held.mach * sound_speed),
            SpeedSet::Ve => self.set_vtrue_fps(held.ve * (STD_DAY_SL_DENSITY / rho).sqrt()),
            _ => {}
        }
    }

    /// `SetAltitudeAGLFtIC`. With a geocentric latitude the geodetic
    /// latitude and altitude are iterated so that the geocentric latitude
    /// is kept.
    pub fn set_altitude_agl_ft(&mut self, env: Environment<'_>, agl: f64) {
        let held = self.held_speed(env);
        match self.last_latitude_set {
            LatitudeSet::Geodetic => env.inertial.set_altitude_agl(&mut self.position, agl),
            LatitudeSet::Geocentric => self.solve_agl_at_geocentric_latitude(env.inertial, agl),
        }
        let altitude_asl = self.altitude_asl_ft();
        self.restore_speed(env, &held, altitude_asl);
        self.last_altitude_set = AltitudeSet::Agl;
    }

    fn solve_agl_at_geocentric_latitude(&mut self, inertial: &Inertial, agl: f64) {
        let (a, b) = (inertial.a, inertial.b);
        let e2 = 1.0 - b * b / (a * a);
        let tanlat = libm::tan(self.position.latitude());
        let mut n = e2;
        let mut prev_n = 1.0;
        let mut iter = 0;
        let longitude = self.position.longitude();
        let mut alt = self.position.geod_altitude();
        let mut h = -2.0 * a.max(b);
        while ((n - prev_n).abs() > 1E-15 || (h - agl).abs() > 1E-10) && iter < 10 {
            let geod_lat = libm::atan(tanlat / (1.0 - n));
            self.position
                .set_position_geodetic(longitude, geod_lat, alt);
            h = inertial.altitude_agl(&self.position);
            alt += agl - h;
            let sin_geod_lat = libm::sin(geod_lat);
            let big_n = a / (1.0 - e2 * sin_geod_lat * sin_geod_lat).sqrt();
            prev_n = n;
            n = e2 * big_n / (big_n + alt);
            iter += 1;
        }
    }

    /// `SetAltitudeASLFtIC`. With a geodetic latitude the geodetic altitude
    /// that puts the position `alt` above sea level is solved for.
    ///
    /// The speed set last is restored at the atmosphere of the geodetic
    /// altitude, which is what JSBSim does, not at the altitude above sea
    /// level.
    pub fn set_altitude_asl_ft(&mut self, env: Environment<'_>, alt: f64) {
        let held = self.held_speed(env);
        match self.last_latitude_set {
            LatitudeSet::Geodetic => {
                let geod_latitude = self.position.geod_latitude_rad();
                let geod_alt = geodetic_altitude_for_asl(env.inertial, geod_latitude, alt);
                let longitude = self.position.longitude();
                self.position
                    .set_position_geodetic(longitude, geod_latitude, geod_alt);
            }
            LatitudeSet::Geocentric => {
                let slr = self.position.sea_level_radius();
                self.position.set_radius(slr + alt);
            }
        }
        let altitude_asl = self.position.geod_altitude();
        self.restore_speed(env, &held, altitude_asl);
        self.last_altitude_set = AltitudeSet::Asl;
    }

    /// `SetTerrainElevationFtIC`: a height above ground set last is kept.
    pub fn set_terrain_elevation_ft(
        &mut self,
        atmosphere: &crate::models::atmosphere::Atmosphere,
        inertial: &mut Inertial,
        elev: f64,
    ) {
        let agl = inertial.altitude_agl(&self.position);
        inertial.terrain_elevation = elev;
        if self.last_altitude_set == AltitudeSet::Agl {
            let env = Environment {
                atmosphere,
                inertial,
            };
            self.set_altitude_agl_ft(env, agl);
        }
    }

    /// `SetGeodLatitudeRadIC`: the altitude kind set last is kept.
    pub fn set_geod_latitude_rad(&mut self, env: Environment<'_>, geod_latitude: f64) {
        let lon = self.position.longitude();
        self.last_latitude_set = LatitudeSet::Geodetic;
        match self.last_altitude_set {
            AltitudeSet::Agl => {
                let agl = self.altitude_agl_ft(env);
                self.position.set_position_geodetic(lon, geod_latitude, 0.);
                env.inertial.set_altitude_agl(&mut self.position, agl);
            }
            AltitudeSet::Asl => {
                let asl = self.altitude_asl_ft();
                self.position.set_position_geodetic(lon, geod_latitude, 0.);
                self.set_altitude_asl_ft(env, asl);
            }
        }
    }

    /// `SetGeodLatitudeDegIC`.
    pub fn set_geod_latitude_deg(&mut self, env: Environment<'_>, glat: f64) {
        self.set_geod_latitude_rad(env, glat * DEGTORAD);
    }

    /// `SetLatitudeRadIC`: geocentric; the altitude kind set last is kept.
    pub fn set_latitude_rad(&mut self, env: Environment<'_>, lat: f64) {
        self.last_latitude_set = LatitudeSet::Geocentric;
        match self.last_altitude_set {
            AltitudeSet::Agl => {
                let altitude = self.altitude_agl_ft(env);
                self.position.set_latitude(lat);
                self.set_altitude_agl_ft(env, altitude);
            }
            AltitudeSet::Asl => {
                let altitude = self.altitude_asl_ft();
                self.position.set_latitude(lat);
                self.set_altitude_asl_ft(env, altitude);
            }
        }
    }

    /// `SetLatitudeDegIC`.
    pub fn set_latitude_deg(&mut self, env: Environment<'_>, lat: f64) {
        self.set_latitude_rad(env, lat * DEGTORAD);
    }

    /// `SetLongitudeRadIC`: a height above ground set last is kept.
    pub fn set_longitude_rad(&mut self, env: Environment<'_>, lon: f64) {
        match self.last_altitude_set {
            AltitudeSet::Agl => {
                let altitude = self.altitude_agl_ft(env);
                self.position.set_longitude(lon);
                self.set_altitude_agl_ft(env, altitude);
            }
            AltitudeSet::Asl => self.position.set_longitude(lon),
        }
    }

    /// `SetLongitudeDegIC`.
    pub fn set_longitude_deg(&mut self, env: Environment<'_>, lon: f64) {
        self.set_longitude_rad(env, lon * DEGTORAD);
    }

    /// `GetAltitudeASLFtIC`: radius above the sea level radius.
    #[must_use]
    pub fn altitude_asl_ft(&self) -> f64 {
        self.position.radius() - self.position.sea_level_radius()
    }

    /// `GetAltitudeAGLFtIC`.
    #[must_use]
    pub fn altitude_agl_ft(&self, env: Environment<'_>) -> f64 {
        env.inertial.altitude_agl(&self.position)
    }

    /// `GetTerrainElevationFtIC`: geodetic altitude of the terrain below.
    #[must_use]
    pub fn terrain_elevation_ft(&self, env: Environment<'_>) -> f64 {
        env.inertial.contact(&self.position).point.geod_altitude()
    }

    /// `GetLatitudeRadIC`: geocentric.
    #[must_use]
    pub fn latitude_rad(&self) -> f64 {
        self.position.latitude()
    }

    /// `GetLatitudeDegIC`.
    #[must_use]
    pub fn latitude_deg(&self) -> f64 {
        self.position.latitude_deg()
    }

    /// `GetGeodLatitudeRadIC`.
    #[must_use]
    pub fn geod_latitude_rad(&self) -> f64 {
        self.position.geod_latitude_rad()
    }

    /// `GetGeodLatitudeDegIC`.
    #[must_use]
    pub fn geod_latitude_deg(&self) -> f64 {
        self.position.geod_latitude_deg()
    }

    /// `GetLongitudeRadIC`.
    #[must_use]
    pub fn longitude_rad(&self) -> f64 {
        self.position.longitude()
    }

    /// `GetLongitudeDegIC`.
    #[must_use]
    pub fn longitude_deg(&self) -> f64 {
        self.position.longitude_deg()
    }
}

/// The geodetic altitude at `geod_latitude` whose position is `alt` above
/// the sea level radius, iterating Stevens & Lewis 1.6-14 on the
/// geocentric latitude; the tangent or the cotangent is used, whichever is
/// bounded.
fn geodetic_altitude_for_asl(inertial: &Inertial, geod_latitude: f64, alt: f64) -> f64 {
    let (a, b) = (inertial.a, inertial.b);
    let e2 = 1.0 - b * b / (a * a);
    let cos_geod_lat = libm::cos(geod_latitude);
    let sin_geod_lat = libm::sin(geod_latitude);
    let big_n = a / (1.0 - e2 * sin_geod_lat * sin_geod_lat).sqrt();
    let mut n = e2;
    let mut prev_n = 1.0;
    let mut iter = 0;
    if cos_geod_lat > sin_geod_lat.abs() {
        let tan_geod_lat = sin_geod_lat / cos_geod_lat;
        let x0 = big_n * e2 * cos_geod_lat;
        let mut x = 0.0;
        while (n - prev_n).abs() > 1E-15 && iter < 10 {
            let tan_lat = (1.0 - n) * tan_geod_lat;
            let cos2_lat = 1. / (1. + tan_lat * tan_lat);
            let slr = b / (1. - e2 * cos2_lat).sqrt();
            let r = slr + alt;
            x = r * cos2_lat.sqrt();
            prev_n = n;
            n = x0 / x;
            iter += 1;
        }
        x / cos_geod_lat - big_n
    } else {
        let cotan_geod_lat = cos_geod_lat / sin_geod_lat;
        let z0 = big_n * e2 * sin_geod_lat;
        let mut z = 0.0;
        while (n - prev_n).abs() > 1E-15 && iter < 10 {
            let cotan_lat = cotan_geod_lat / (1.0 - n);
            let sin2_lat = 1. / (1. + cotan_lat * cotan_lat);
            let cos2_lat = 1. - sin2_lat;
            let slr = b / (1. - e2 * cos2_lat).sqrt();
            let r = slr + alt;
            z = r * sign(cotan_lat) * sin2_lat.sqrt();
            prev_n = n;
            n = z0 / (z0 + z);
            iter += 1;
        }
        z / sin_geod_lat - big_n * (1.0 - e2)
    }
}
