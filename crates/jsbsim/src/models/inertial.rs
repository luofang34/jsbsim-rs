// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// FGGroundCallback: Copyright (C) 2004 Mathias Froehlich.
// Derived from JSBSim src/models/FGInertial.{h,cpp} and
// src/input_output/FGGroundCallback.cpp (FGDefaultGroundCallback).
//
//! The planet: shape, rotation, gravity and the default flat-terrain
//! ground callback.

use super::base::ModelBase;
use crate::Error;
use crate::math::{ColumnVector3, E_X, E_Y, E_Z, Location};
use crate::xml::Element;

/// `simulation/gravity-model` for spherical gravity, GM/r^2 to the centre.
pub const GRAVITY_STANDARD: i32 = 0;
/// `simulation/gravity-model` for WGS84 gravity with the J2 term.
pub const GRAVITY_WGS84: i32 = 1;

/// `gAccelReference`: standard gravity in ft/s^2.
pub const STANDARD_GRAVITY: f64 = 9.80665 / crate::units::FTTOM;

/// `FGInertial`.
#[derive(Debug, Clone)]
pub struct Inertial {
    /// Scheduling.
    pub base: ModelBase,
    /// Planet name.
    pub name: String,
    /// Semi-major axis, ft.
    pub a: f64,
    /// Semi-minor axis, ft.
    pub b: f64,
    /// Gravitational parameter, ft^3/s^2.
    pub gm: f64,
    /// J2 coefficient.
    pub j2: f64,
    /// Planet rotation vector, rad/s.
    pub omega_planet: ColumnVector3,
    /// Gravity model; JSBSim stores any integer written, and other values
    /// leave the gravity unchanged.
    pub grav_type: i32,
    /// Gravity acceleration at the last position, ECEF, ft/s^2.
    pub grav_accel: ColumnVector3,
    /// Position the last gravity was computed at (`in.Position`).
    pub position: Location,
    /// Terrain elevation of the default ground callback, ft.
    pub terrain_elevation: f64,
    /// Simulation time given to the ground callback.
    pub time: f64,
}

impl Default for Inertial {
    fn default() -> Self {
        let a = 20925646.32546;
        let b = 20855486.5951;
        Self {
            base: ModelBase::default(),
            name: "Earth".to_owned(),
            a,
            b,
            gm: 14.0764417572E15,
            j2: 1.08262982E-03,
            omega_planet: ColumnVector3::new(0.0, 0.0, 0.00007292115),
            grav_type: GRAVITY_WGS84,
            grav_accel: ColumnVector3::zero(),
            position: Location::default(),
            terrain_elevation: 0.0,
            time: 0.0,
        }
    }
}

/// What the ground callback reports about the terrain under a location.
#[derive(Debug, Clone)]
pub struct Contact {
    /// Height of the location above the terrain, ft.
    pub agl: f64,
    /// Terrain point below the location.
    pub point: Location,
    /// Terrain normal, ECEF.
    pub normal: ColumnVector3,
    /// Terrain velocity, ECEF.
    pub velocity: ColumnVector3,
    /// Terrain angular velocity, ECEF.
    pub angular_velocity: ColumnVector3,
}

impl Inertial {
    /// `Load(<planet>)`: overrides of the planet constants.
    pub fn load(&mut self, el: &Element) -> Result<(), Error> {
        self.name = el.attribute("name").to_owned();
        if el.find_element("semimajor_axis").is_some() {
            self.a = el.find_element_value_as_number_convert_to("semimajor_axis", "FT")?;
        } else if el.find_element("equatorial_radius").is_some() {
            self.a = el.find_element_value_as_number_convert_to("equatorial_radius", "FT")?;
        }
        if el.find_element("semiminor_axis").is_some() {
            self.b = el.find_element_value_as_number_convert_to("semiminor_axis", "FT")?;
        } else if el.find_element("polar_radius").is_some() {
            self.b = el.find_element_value_as_number_convert_to("polar_radius", "FT")?;
        }
        if el.find_element("rotation_rate").is_some() {
            let rate = el.find_element_value_as_number_convert_to("rotation_rate", "RAD/SEC")?;
            self.omega_planet = ColumnVector3::new(0.0, 0.0, rate);
        }
        if el.find_element("GM").is_some() {
            self.gm = el.find_element_value_as_number_convert_to("GM", "FT3/SEC2")?;
        }
        if el.find_element("J2").is_some() {
            self.j2 = el.find_element_value_as_number("J2")?;
        }
        Ok(())
    }

    /// `Run`: gravity at the current position.
    pub fn run(&mut self, holding: bool) {
        if self.base.skip() || holding {
            return;
        }
        match self.grav_type {
            GRAVITY_STANDARD => {
                let radius = self.position.radius();
                self.grav_accel = -(self.g_accel(radius) / radius) * self.position.ecef();
            }
            GRAVITY_WGS84 => self.grav_accel = self.gravity_j2(&self.position),
            _ => {}
        }
    }

    /// `GetGAccel(r)`.
    #[must_use]
    pub fn g_accel(&self, r: f64) -> f64 {
        self.gm / (r * r)
    }

    /// `GetGravityJ2(position)`.
    #[must_use]
    pub fn gravity_j2(&self, position: &Location) -> ColumnVector3 {
        let r = position.radius();
        let sin_lat = libm::sin(position.latitude());
        let adivr = self.a / r;
        let pre_common = 1.5 * self.j2 * adivr * adivr;
        let xy = 1.0 - 5.0 * (sin_lat * sin_lat);
        let z = 3.0 - 5.0 * (sin_lat * sin_lat);
        let gm_over_r2 = self.gm / (r * r);
        ColumnVector3::new(
            -gm_over_r2 * ((1.0 + (pre_common * xy)) * position.entry(E_X) / r),
            -gm_over_r2 * ((1.0 + (pre_common * xy)) * position.entry(E_Y) / r),
            -gm_over_r2 * ((1.0 + (pre_common * z)) * position.entry(E_Z) / r),
        )
    }

    /// `FGDefaultGroundCallback::GetAGLevel`: flat terrain at the
    /// configured elevation above the ellipsoid.
    #[must_use]
    pub fn contact(&self, location: &Location) -> Contact {
        let mut l = location.clone();
        l.set_ellipse(self.a, self.b);
        let latitude = l.geod_latitude_rad();
        let cos_lat = libm::cos(latitude);
        let longitude = l.longitude();
        let normal = ColumnVector3::new(
            cos_lat * libm::cos(longitude),
            cos_lat * libm::sin(longitude),
            libm::sin(latitude),
        );
        let mut point = Location::default();
        point.set_ellipse(self.a, self.b);
        point.set_position_geodetic(longitude, latitude, self.terrain_elevation);
        Contact {
            agl: l.geod_altitude() - self.terrain_elevation,
            point,
            normal,
            velocity: ColumnVector3::zero(),
            angular_velocity: ColumnVector3::zero(),
        }
    }

    /// `GetAltitudeAGL(location)`.
    #[must_use]
    pub fn altitude_agl(&self, location: &Location) -> f64 {
        self.contact(location).agl
    }

    /// `SetAltitudeAGL(location, h)`: moves `location` to `h` above the
    /// terrain along its geodetic vertical.
    pub fn set_altitude_agl(&self, location: &mut Location, altitude_agl: f64) {
        let ground = self.contact(location).point.geod_altitude();
        let longitude = location.longitude();
        let geod_lat = location.geod_latitude_rad();
        location.set_position_geodetic(longitude, geod_lat, ground + altitude_agl);
    }

    /// `SetGravityType`.
    pub fn set_gravity_type(&mut self, gt: i32) {
        if gt == GRAVITY_STANDARD && self.a != self.b {
            tracing::warn!("Standard gravity model has been set for a non-spherical planet");
        }
        if gt == GRAVITY_WGS84 && self.j2 == 0.0 {
            tracing::warn!(
                "WGS84 gravity model has been set without specifying the J2 gravitational constant."
            );
        }
        self.grav_type = gt;
    }
}

/// `FGInertial::bind`.
pub fn bind(b: &mut crate::fdm::Binder<'_>) -> Result<(), Error> {
    b.getter("inertial/sea-level-radius_ft", |m| {
        m.inertial.position.sea_level_radius()
    })?;
    b.property(
        "simulation/gravity-model",
        crate::fdm::Kind::Int,
        |m| f64::from(m.inertial.grav_type),
        |m, v| m.inertial.set_gravity_type(v as i32),
    )?;
    Ok(())
}
