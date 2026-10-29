// SPDX-License-Identifier: LGPL-2.1-or-later
// FGLGear: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/FGLGear.cpp (GetBodyForces,
// ComputeGroundFrame, ComputeSteeringAngle, ComputeVerticalStrutForce)
// and src/models/propulsion/FGForce.cpp (GetBodyForces).
//
//! `FGLGear::GetBodyForces`, in the phases the executive runs it in: up to
//! the compression speed, where a `<strut_force>` function reads the
//! contact's properties, then the strut and friction, then what every
//! frame ends with.

use super::contact::{Contact, ContactType, DampType};
use super::force::FrictionType;
use super::{Inputs, Surface};
use crate::Error;
use crate::functions::FunctionSet;
use crate::math::{ColumnVector3, E_X, E_Y, E_Z, Matrix33};
use crate::models::inertial::Inertial;
use crate::units::{INCHTOFT, sign};

/// What the executive tells every contact about the frame.
#[derive(Debug, Clone, Copy)]
pub struct FrameContext {
    /// `GetTrimStatus`.
    pub trim_status: bool,
    /// `GetSimTime`, s.
    pub sim_time: f64,
    /// `IntegrationSuspended`.
    pub integration_suspended: bool,
}

/// A contact compressed against the ground this frame, between
/// [`Contact::begin_frame`] and [`Contact::finish_compressed`].
#[derive(Debug, Clone, Copy)]
pub struct Compressed {
    /// `vWhlContactVec`: the contact patch relative to the CG, body frame,
    /// ft.
    contact_vec: ColumnVector3,
}

/// `FGLGear::Ts2b`: structural to body, inches to feet.
fn ts2b() -> Matrix33 {
    Matrix33::new(-INCHTOFT, 0., 0., 0., INCHTOFT, 0., 0., 0., -INCHTOFT)
}

/// `FGLGear::Tb2s`: body to structural, feet to inches.
fn tb2s() -> Matrix33 {
    Matrix33::new(
        -1. / INCHTOFT,
        0.,
        0.,
        0.,
        1. / INCHTOFT,
        0.,
        0.,
        0.,
        -1. / INCHTOFT,
    )
}

/// `std::max(a, b)`, which is `a < b ? b : a`: unlike [`f64::max`], it
/// keeps `a` when either is NaN and when both are zeros.
pub(super) fn cpp_max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// `std::min(a, b)`, which is `b < a ? b : a`.
fn cpp_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

impl Contact {
    /// `GetBodyLocation`: the contact relative to the CG of `input`, body
    /// frame, ft.
    #[must_use]
    pub fn body_location(&self, input: &Inputs) -> ColumnVector3 {
        ts2b() * (self.location - input.xyz_cg)
    }

    /// `GetBodyForces` up to the compression speed. Returns what the rest
    /// of a compressed frame needs; otherwise the frame is complete but for
    /// [`Contact::end_frame`].
    pub fn begin_frame(
        &mut self,
        input: &Inputs,
        inertial: &Inertial,
        surface: &Surface,
        bump: f64,
        trim_status: bool,
    ) -> Option<Compressed> {
        let mut gear_pos = 1.0;
        self.force.normal_force = ColumnVector3::zero();
        let body = ts2b() * (self.location - input.xyz_cg);
        self.state.local_gear = input.tb2l * body;
        let gear_loc = input.location.local_to_location(self.state.local_gear);
        let contact = inertial.contact(&gear_loc);
        self.state.agl = cpp_max(contact.agl, 0.0);
        if self.retractable {
            gear_pos = self.gear_unit_pos(input.fcs_gear_pos);
        }
        if gear_pos > 0.99 {
            let mut height = contact.agl;
            if !trim_status {
                height -= bump;
            }
            self.surface.static_factor = surface.static_factor;
            self.surface.rolling_factor = surface.rolling_factor;
            self.surface.maximum_force = surface.maximum_force;
            self.surface.bumpiness = surface.bumpiness;
            self.surface.solid = surface.solid;
            return self.gear_down(input, &contact, body, height, gear_pos, trim_status);
        }
        None
    }

    /// `GetBodyForces` with the gear down, from the height test.
    fn gear_down(
        &mut self,
        input: &Inputs,
        contact: &crate::models::inertial::Contact,
        body: ColumnVector3,
        height: f64,
        gear_pos: f64,
        trim_status: bool,
    ) -> Option<Compressed> {
        let mut displacement = ColumnVector3::zero();
        let mut lgear_proj = 1.0;
        if height < 0.0 {
            self.state.wow = true;
            self.force.ground_normal = input.tec2b * contact.normal;
            let normal_z = (input.tec2l * contact.normal)[E_Z];
            lgear_proj = -(self.t_gear.transposed() * self.force.ground_normal)[E_Z];
            let s = &mut self.state;
            match self.kind {
                ContactType::Bogey => {
                    if self.surface.solid {
                        s.compress_length = if lgear_proj > 0.0 {
                            height * normal_z / lgear_proj
                        } else {
                            0.0
                        };
                        displacement = self.t_gear * ColumnVector3::new(0., 0., -s.compress_length);
                    } else {
                        s.wow = false;
                    }
                }
                ContactType::Structure => {
                    let n = contact.normal;
                    s.compress_length = height * normal_z / crate::math::dot_product(n, n);
                    displacement = s.compress_length * self.force.ground_normal;
                }
            }
        } else {
            self.state.wow = false;
        }
        if !self.state.wow {
            self.not_compressed(input, gear_pos);
            return None;
        }
        let contact_vec = body + displacement;
        self.acting_location = self.location + tb2s() * displacement;
        let mut body_wheel_vel = input.pqr.cross(contact_vec);
        body_wheel_vel += input.uvw - input.tec2b * contact.velocity;
        self.state.wheel_vel = self.t_gear.transposed() * body_wheel_vel;
        self.initialize_reporting(input);
        self.compute_steering_angle();
        self.compute_ground_frame();
        self.force.ground_wheel_vel = self.force.t_ground.transposed() * body_wheel_vel;
        self.compute_compress_speed(input, trim_status, lgear_proj);
        Some(Compressed { contact_vec })
    }

    /// The "gear is NOT compressed" branch of `GetBodyForces`.
    fn not_compressed(&mut self, input: &Inputs, gear_pos: f64) {
        let s = &mut self.state;
        s.compress_length = 0.0;
        s.compress_speed = 0.0;
        s.wheel_slip = 0.0;
        s.strut_force = 0.0;
        let m = &mut self.force.multipliers;
        m.roll.value = 0.0;
        m.side.value = 0.0;
        m.dynamic.value = 0.0;
        self.steering.angle *= cpp_max(gear_pos - 0.8, 0.0) / 0.2;
        self.reset_reporting(input);
    }

    fn compute_compress_speed(&mut self, input: &Inputs, trim_status: bool, lgear_proj: f64) {
        let s = &mut self.state;
        if trim_status || input.total_delta_t == 0.0 {
            s.compress_speed = 0.0;
            return;
        }
        s.compress_speed = -self.force.ground_wheel_vel[E_Z];
        if self.kind == ContactType::Bogey {
            s.compress_speed /= lgear_proj;
        }
        let max_compress_speed = s.compress_length / input.total_delta_t;
        if s.compress_speed.abs() > max_compress_speed {
            s.compress_speed = sign(s.compress_speed) * max_compress_speed;
        }
    }

    /// The rest of a compressed frame: `strut` is the `<strut_force>`
    /// function's value when the contact has one; `functions` holds the
    /// cornering table; the friction constraints are registered in
    /// `registry` under `index`, the contact's.
    pub fn finish_compressed(
        &mut self,
        compressed: Compressed,
        strut: Option<f64>,
        input: &Inputs,
        functions: &FunctionSet,
        registry: &mut Vec<(usize, FrictionType)>,
        index: usize,
    ) -> Result<(), Error> {
        self.compute_vertical_strut_force(strut);
        if self.kind == ContactType::Bogey {
            self.compute_slip_angle();
            self.compute_brake_force_coefficient(input);
            self.compute_side_force_coefficient(functions)?;
        }
        self.compute_jacobian(compressed.contact_vec, registry, index);
        Ok(())
    }

    /// `ComputeSteeringAngle`.
    fn compute_steering_angle(&mut self) {
        let v = &self.state.wheel_vel;
        if self.steering.castered && v.magnitude2(E_X, E_Y) > 0.1 {
            self.steering.angle = libm::atan2(v[E_Y], v[E_X].abs());
        }
    }

    /// `ComputeGroundFrame`: roll, side and normal directions as the
    /// columns of `mT`.
    fn compute_ground_frame(&mut self) {
        let a = self.steering.angle;
        let normal = self.force.ground_normal;
        let (sin_a, cos_a) = libm::sincos(a);
        let mut roll = self.t_gear * ColumnVector3::new(cos_a, sin_a, 0.);
        let mut side = normal.cross(roll);
        roll -= crate::math::dot_product(roll, normal) * normal;
        roll.normalize();
        side.normalize();
        let t = &mut self.force.t_ground;
        for (column, v) in [(E_X, roll), (E_Y, side), (E_Z, normal)] {
            t[(E_X, column)] = v[E_X];
            t[(E_Y, column)] = v[E_Y];
            t[(E_Z, column)] = v[E_Z];
        }
    }

    /// `ComputeVerticalStrutForce`.
    fn compute_vertical_strut_force(&mut self, function: Option<f64>) {
        let s = &mut self.state;
        if let Some(value) = function {
            s.strut_force = cpp_min(value, 0.0);
        } else {
            let st = &self.strut;
            let spring_force = -s.compress_length * st.k_spring;
            let cs = s.compress_speed;
            let damp_force = if cs >= 0.0 {
                match st.damp {
                    DampType::Linear => -cs * st.b_damp,
                    DampType::Square => -cs * cs * st.b_damp,
                }
            } else {
                match st.damp_rebound {
                    DampType::Linear => -cs * st.b_damp_rebound,
                    DampType::Square => cs * cs * st.b_damp_rebound,
                }
            };
            s.strut_force = cpp_min(spring_force + damp_force, 0.0);
            if s.strut_force > self.surface.maximum_force {
                s.strut_force = self.surface.maximum_force;
                s.compress_length = -s.strut_force / st.k_spring;
            }
        }
        self.force.normal_force[E_Z] = match self.kind {
            ContactType::Bogey => {
                s.strut_force / (self.t_gear.transposed() * self.force.ground_normal)[E_Z]
            }
            ContactType::Structure => -s.strut_force,
        };
        let r = &mut self.reporting;
        r.max_strut_force = cpp_max(r.max_strut_force, s.strut_force.abs());
        r.max_strut_travel = cpp_max(r.max_strut_travel, s.compress_length.abs());
    }

    /// What every frame of `GetBodyForces` ends with, whether or not the
    /// contact is compressed, including `FGForce::GetBodyForces`;
    /// `structural_to_body` is `FGMassBalance::StructuralToBody`.
    pub fn end_frame(
        &mut self,
        input: &Inputs,
        frame: &FrameContext,
        structural_to_body: impl Fn(ColumnVector3) -> ColumnVector3,
    ) {
        let s = &mut self.state;
        if !s.wow {
            s.wheel_vel[E_X] -= 13.0 * input.total_delta_t;
            if s.wheel_vel[E_X] < 0.0 {
                s.wheel_vel[E_X] = 0.0;
            }
        }
        if !frame.trim_status {
            self.report_takeoff_or_landing(input, frame.sim_time);
            if self.state.wow && self.state.last_wow {
                self.crash_detect(frame);
            }
            self.state.last_wow = self.state.wow;
        }
        let f = &mut self.force;
        f.body_force = f.t_ground * f.normal_force;
        let arm = structural_to_body(self.acting_location);
        f.moment = ColumnVector3::zero() + arm.cross(f.body_force);
    }
}
