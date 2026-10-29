// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGInitialCondition.cpp (Load_v2).
//
//! Initialization files of format 2: position in the ECEF or ECI frame,
//! orientation and velocity in the local, body, ECEF or ECI frame, and body
//! rates. The planet position angle relates ECI to ECEF. The orientation
//! and velocity are stored directly, not through the setters.

use super::load::LoadOutcome;
use super::{Environment, InitialCondition, SpeedSet};
use crate::Error;
use crate::math::{ColumnVector3, E_EAST, E_NORTH, Matrix33, Quaternion};
use crate::models::atmosphere::Atmosphere;
use crate::models::inertial::Inertial;
use crate::xml::Element;

/// ECI to ECEF for the planet position angle `epa`.
fn inertial_to_ecef(epa: f64) -> Matrix33 {
    let (s, c) = libm::sincos(epa);
    Matrix33::new(c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0)
}

fn frame_of(el: &Element) -> String {
    el.attribute("frame").to_ascii_lowercase()
}

impl InitialCondition {
    /// `Load_v2`.
    pub(super) fn load_v2(
        &mut self,
        document: &Element,
        atmosphere: &Atmosphere,
        inertial: &mut Inertial,
    ) -> Result<LoadOutcome, Error> {
        let mut outcome = LoadOutcome {
            result: true,
            planet_rotation_changed: false,
        };
        // Both names are accepted; the later one wins.
        for name in ["earth_position_angle", "planet_position_angle"] {
            if document.find_element(name).is_some() {
                self.epa = document.find_element_value_as_number_convert_to(name, "RAD")?;
            }
        }
        let ti2ec = inertial_to_ecef(self.epa);
        if document.find_element("planet_rotation_rate").is_some() {
            let rate =
                document.find_element_value_as_number_convert_to("planet_rotation_rate", "RAD")?;
            inertial.omega_planet = ColumnVector3::new(0.0, 0.0, rate);
            outcome.planet_rotation_changed = true;
        }
        let omega = inertial.omega_planet;
        if document.find_element("elevation").is_some() {
            inertial.terrain_elevation =
                document.find_element_value_as_number_convert_to("elevation", "FT")?;
        }
        let env = Environment::new(atmosphere, inertial);
        match document.find_element("position") {
            Some(el) => {
                if !self.load_v2_position(el, env, &ti2ec)? {
                    outcome.result = false;
                }
            }
            None => {
                tracing::error!(
                    "{}: Initial position not specified in this initialization file.",
                    document.location()
                );
                outcome.result = false;
            }
        }
        if let Some(el) = document.find_element("orientation")
            && !self.load_v2_orientation(el, &ti2ec)?
        {
            outcome.result = false;
        }
        if !self.load_v2_velocity(document.find_element("velocity"), omega, &ti2ec)? {
            outcome.result = false;
        }
        if !self.load_v2_attitude_rate(document.find_element("attitude_rate"), omega, &ti2ec)? {
            outcome.result = false;
        }
        Ok(outcome)
    }

    fn load_v2_position(
        &mut self,
        el: &Element,
        env: Environment<'_>,
        ti2ec: &Matrix33,
    ) -> Result<bool, Error> {
        match frame_of(el).as_str() {
            "eci" => {
                let eci = el.find_element_triplet_convert_to("FT")?;
                self.position.assign_ecef(*ti2ec * eci);
                Ok(true)
            }
            "ecef" => {
                if ["x", "y", "z"].iter().any(|n| el.find_element(n).is_some()) {
                    let ecef = el.find_element_triplet_convert_to("FT")?;
                    self.position.assign_ecef(ecef);
                    return Ok(true);
                }
                if el.find_element("longitude").is_some() {
                    let lon = el.find_element_value_as_number_convert_to("longitude", "RAD")?;
                    self.set_longitude_rad(env, lon);
                }
                if el.find_element("radius").is_some() {
                    let radius = el.find_element_value_as_number_convert_to("radius", "FT")?;
                    self.position.set_radius(radius);
                } else if el.find_element("altitudeAGL").is_some() {
                    let agl = el.find_element_value_as_number_convert_to("altitudeAGL", "FT")?;
                    self.set_altitude_agl_ft(env, agl);
                } else if el.find_element("altitudeMSL").is_some() {
                    let asl = el.find_element_value_as_number_convert_to("altitudeMSL", "FT")?;
                    self.set_altitude_asl_ft(env, asl);
                } else {
                    tracing::error!(
                        "{}: No altitude or radius initial condition is given.",
                        el.location()
                    );
                    return Ok(false);
                }
                self.load_latitude(el, env)
            }
            _ => {
                tracing::error!(
                    "{}: Neither ECI nor ECEF frame is specified for initial position.",
                    el.location()
                );
                Ok(false)
            }
        }
    }

    /// Euler angles of the body relative to the ECI, ECEF or local frame,
    /// stored as the local-to-body quaternion.
    fn load_v2_orientation(&mut self, el: &Element, ti2ec: &Matrix33) -> Result<bool, Error> {
        let frame = frame_of(el);
        let orient = el.find_element_triplet_convert_to("RAD")?;
        match frame.as_str() {
            "eci" => {
                // Q_b/l = Q_i/l * Q_b/i
                let mut i2body = Quaternion::from_euler_vector(orient);
                i2body.normalize();
                let tec2i = ti2ec.transposed();
                let mut local2i = Quaternion::from_matrix(&(tec2i * self.position.tl2ec()));
                local2i.normalize();
                self.orientation = &local2i * &i2body;
            }
            "ecef" => {
                // Q_b/l = Q_e/l * Q_b/e
                let mut ec2body = Quaternion::from_euler_vector(orient);
                ec2body.normalize();
                let mut local2ec = Quaternion::from_matrix(&self.position.tl2ec());
                local2ec.normalize();
                self.orientation = &local2ec * &ec2body;
            }
            "local" => self.orientation = Quaternion::from_euler_vector(orient),
            _ => {
                tracing::error!(
                    "{}: Orientation frame type: \"{frame}\" is not supported!",
                    el.location()
                );
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// The ground velocity, zero when no element is given; then the true
    /// airspeed is the ground speed (no wind) and alpha and beta follow.
    fn load_v2_velocity(
        &mut self,
        el: Option<&Element>,
        omega: ColumnVector3,
        ti2ec: &Matrix33,
    ) -> Result<bool, Error> {
        let tec2l = self.position.tec2l();
        let tb2l = self.orientation.t_inv();
        let mut result = true;
        match el {
            Some(el) => {
                let frame = frame_of(el);
                let v = el.find_element_triplet_convert_to("FT/SEC")?;
                match frame.as_str() {
                    "eci" => {
                        let omega_cross_r = omega.cross(ti2ec.transposed() * self.position.ecef());
                        self.uvw_ned = tec2l * (v - omega_cross_r);
                        self.last_speed_set = SpeedSet::Ned;
                    }
                    "ecef" => {
                        self.uvw_ned = tec2l * v;
                        self.last_speed_set = SpeedSet::Ned;
                    }
                    "local" => {
                        self.uvw_ned = v;
                        self.last_speed_set = SpeedSet::Ned;
                    }
                    "body" => {
                        self.uvw_ned = tb2l * v;
                        self.last_speed_set = SpeedSet::Uvw;
                    }
                    _ => {
                        tracing::error!(
                            "{}: Velocity frame type: \"{frame}\" is not supported!",
                            el.location()
                        );
                        result = false;
                    }
                }
            }
            None => self.uvw_ned = ColumnVector3::zero(),
        }
        self.vt = self.uvw_ned.magnitude();
        self.calc_aero_angles(self.uvw_ned);
        Ok(result)
    }

    /// Body rates given relative to the ECI, ECEF, local or body frame,
    /// stored relative to ECEF in the body frame; zero when absent or
    /// without a frame.
    fn load_v2_attitude_rate(
        &mut self,
        el: Option<&Element>,
        omega: ColumnVector3,
        ti2ec: &Matrix33,
    ) -> Result<bool, Error> {
        let Some(el) = el else {
            self.pqr_body = ColumnVector3::zero();
            return Ok(true);
        };
        let frame = frame_of(el);
        let tl2b = self.orientation.t();
        let rate = el.find_element_triplet_convert_to("RAD/SEC")?;
        match frame.as_str() {
            "eci" => {
                let ti2l = self.position.tec2l() * *ti2ec;
                self.pqr_body = tl2b * ti2l * (rate - omega);
            }
            "ecef" => self.pqr_body = tl2b * self.position.tec2l() * rate,
            "local" => {
                // Rotation rate of the local frame, Stevens and Lewis
                // 1.5-14a.
                let rad_inv = 1.0 / self.position.radius();
                let v = self.uvw_ned;
                let omega_local = ColumnVector3::new(
                    rad_inv * v[E_EAST],
                    -rad_inv * v[E_NORTH],
                    -rad_inv * v[E_EAST] * libm::tan(self.position.latitude()),
                );
                self.pqr_body = tl2b * (rate + omega_local);
            }
            "body" => self.pqr_body = rate,
            "" => self.pqr_body = ColumnVector3::zero(),
            _ => {
                tracing::error!(
                    "{}: Attitude rate frame type: \"{frame}\" is not supported!",
                    el.location()
                );
                return Ok(false);
            }
        }
        Ok(true)
    }
}
