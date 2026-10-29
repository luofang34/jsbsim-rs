// SPDX-License-Identifier: LGPL-2.1-or-later
//! The equations of motion's state, with every integrator history, in a
//! snapshot.

use super::{Frames, Inputs, Orbit, Propagate, VehicleState};
use crate::snapshot::visit_fields;

visit_fields! {
    VehicleState {
        location, uvw, pqr, pqri, q_attitude_local, q_attitude_eci, qtrndot, inertial_position,
        inertial_velocity, dq_pqridot, dq_uvwidot, dq_inertial_velocity, dq_qtrndot,
    }
}
visit_fields! { Inputs { pqridot, uvwidot, delta_t, omega_planet, semi_major, semi_minor, gm } }
visit_fields! {
    Orbit {
        h, inclination, right_ascension, eccentricity, perigee_argument, true_anomaly,
        apoapsis_radius, periapsis_radius, period,
    }
}
visit_fields! {
    Frames { ti2ec, tec2i, tl2ec, tec2l, ti2l, tl2i, ti2b, tb2i, tl2b, tb2l, tec2b, tb2ec }
}
visit_fields! {
    Propagate {
        base, input, state, frames, q_ec2b, vel, epa, integrator_rotational_rate,
        integrator_translational_rate, integrator_rotational_position,
        integrator_translational_position, orbit, local_terrain_velocity,
        local_terrain_angular_velocity,
    }
}
