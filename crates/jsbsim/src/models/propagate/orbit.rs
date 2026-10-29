// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGPropagate.cpp (ComputeOrbitalParameters).
//
//! Orbital elements of the current inertial state.

use std::f64::consts::PI;

use super::Propagate;
use crate::math::{ColumnVector3, E_X, E_Y, E_Z, dot_product};
use crate::units::{RADTODEG, constrain};

impl Propagate {
    pub(super) fn compute_orbital_parameters(&mut self) {
        let z = ColumnVector3::new(0., 0., 1.);
        let mut r = self.state.inertial_position;
        let v = self.state.inertial_velocity;
        let gm = self.input.gm;
        let o = &mut self.orbit;
        let angular_momentum = r.cross(v);
        o.h = angular_momentum.magnitude();
        o.inclination = libm::acos(angular_momentum[E_Z] / o.h) * RADTODEG;
        let mut n;
        if o.inclination.abs() > 1E-8 {
            n = z.cross(angular_momentum);
            o.right_ascension = libm::atan2(n[E_Y], n[E_X]) * RADTODEG;
            n.normalize();
        } else {
            o.right_ascension = 0.0;
            n = ColumnVector3::new(1., 0., 0.);
            o.perigee_argument = 0.0;
        }
        r.normalize();
        let vr = dot_product(r, v);
        let mut e_vector = v.cross(angular_momentum) / gm - r;
        o.eccentricity = e_vector.magnitude();
        if o.eccentricity > 1E-8 {
            e_vector /= o.eccentricity;
            if o.inclination.abs() > 1E-8 {
                o.perigee_argument = libm::acos(dot_product(n, e_vector)) * RADTODEG;
                if e_vector[E_Z] < 0.0 {
                    o.perigee_argument = 360. - o.perigee_argument;
                }
            }
        } else {
            e_vector = ColumnVector3::new(1., 0., 0.);
            o.perigee_argument = 0.0;
        }
        o.true_anomaly = libm::acos(constrain(-1.0, dot_product(e_vector, r), 1.0)) * RADTODEG;
        if vr < 0.0 {
            o.true_anomaly = 360. - o.true_anomaly;
        }
        o.apoapsis_radius = o.h * o.h / (gm * (1.0 - o.eccentricity));
        o.periapsis_radius = o.h * o.h / (gm * (1.0 + o.eccentricity));
        o.period = if o.eccentricity < 1.0 {
            let semimajor = 0.5 * (o.apoapsis_radius + o.periapsis_radius);
            2. * PI * crate::elementary::pow(semimajor, 1.5) / gm.sqrt()
        } else {
            0.0
        };
    }
}
