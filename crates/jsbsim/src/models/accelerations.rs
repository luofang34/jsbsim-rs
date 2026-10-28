// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGAccelerations.{h,cpp} and
// src/math/LagrangeMultiplier.h.
//
//! Translational and rotational accelerations from forces, moments and
//! gravity, including ground friction constraints.

use super::base::ModelBase;
use crate::Error;
use crate::fdm::{Binder, Kind};
use crate::math::{
    Axis, ColumnVector3, E_L, E_M, E_N, E_P, E_Q, E_R, E_U, E_V, E_W, E_X, E_Y, E_Z, Matrix33,
    dot_product,
};
use crate::units::constrain;

/// A ground friction constraint (`LagrangeMultiplier`).
#[derive(Debug, Clone, Copy, Default)]
pub struct LagrangeMultiplier {
    /// Direction of the constraint force, body frame.
    pub force_jacobian: ColumnVector3,
    /// Lever arm from the CG, body frame.
    pub lever_arm: ColumnVector3,
    /// Lower bound of the multiplier.
    pub min: f64,
    /// Upper bound of the multiplier.
    pub max: f64,
    /// The multiplier, carried from frame to frame.
    pub value: f64,
}

/// Inputs loaded by the executive (`FGAccelerations::Inputs`).
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// Inertia tensor.
    pub j: Matrix33,
    /// Its inverse.
    pub jinv: Matrix33,
    /// ECI to body.
    pub ti2b: Matrix33,
    /// Body to ECI.
    pub tb2i: Matrix33,
    /// ECEF to body.
    pub tec2b: Matrix33,
    /// ECEF to ECI.
    pub tec2i: Matrix33,
    /// Total moment, body frame.
    pub moment: ColumnVector3,
    /// Ground moment.
    pub ground_moment: ColumnVector3,
    /// Total force, body frame.
    pub force: ColumnVector3,
    /// Ground force.
    pub ground_force: ColumnVector3,
    /// Gravity, ECEF.
    pub grav_accel: ColumnVector3,
    /// Body rates relative to ECI.
    pub pqri: ColumnVector3,
    /// Body rates relative to ECEF.
    pub pqr: ColumnVector3,
    /// Body velocity relative to ECEF.
    pub uvw: ColumnVector3,
    /// ECI position.
    pub inertial_position: ColumnVector3,
    /// Planet rotation.
    pub omega_planet: ColumnVector3,
    /// Terrain velocity.
    pub terrain_velocity: ColumnVector3,
    /// Terrain angular velocity.
    pub terrain_angular_vel: ColumnVector3,
    /// Frame time step.
    pub delta_t: f64,
    /// Vehicle mass, slugs.
    pub mass: f64,
    /// Ground friction constraints.
    pub multipliers: Vec<LagrangeMultiplier>,
}

/// `FGAccelerations`.
#[derive(Debug, Clone, Default)]
pub struct Accelerations {
    /// Scheduling.
    pub base: ModelBase,
    /// Inputs of the current frame.
    pub input: Inputs,
    /// `simulation/gravitational-torque`.
    pub grav_torque: bool,
    /// Body angular acceleration relative to ECEF.
    pub pqrdot: ColumnVector3,
    /// Body angular acceleration relative to ECI.
    pub pqridot: ColumnVector3,
    /// Body acceleration relative to ECEF.
    pub uvwdot: ColumnVector3,
    /// ECI acceleration.
    pub uvwidot: ColumnVector3,
    /// Specific force, body frame.
    pub body_accel: ColumnVector3,
    /// Friction forces.
    pub friction_forces: ColumnVector3,
    /// Friction moments.
    pub friction_moments: ColumnVector3,
}

impl Accelerations {
    /// `InitModel`.
    pub fn init_model(&mut self) {
        self.base.init();
        self.pqridot = ColumnVector3::zero();
        self.uvwidot = ColumnVector3::zero();
        self.uvwdot = ColumnVector3::zero();
        self.body_accel = ColumnVector3::zero();
    }

    /// `Run`.
    pub fn run(&mut self, holding: bool, hold_down: bool, trim_status: bool) {
        if self.base.skip() || holding {
            return;
        }
        self.calculate_pqrdot(hold_down);
        self.calculate_uvwdot(hold_down, trim_status);
        if !hold_down {
            self.calculate_friction_forces(self.input.delta_t * f64::from(self.base.rate));
        }
    }

    /// `InitializeDerivatives`.
    pub fn initialize_derivatives(&mut self, hold_down: bool, trim_status: bool) {
        self.calculate_pqrdot(hold_down);
        self.calculate_uvwdot(hold_down, trim_status);
        self.calculate_friction_forces(0.);
    }

    fn calculate_pqrdot(&mut self, hold_down: bool) {
        let i = &mut self.input;
        if self.grav_torque {
            let mut r = i.ti2b * i.inertial_position;
            let inv_radius = 1.0 / r.magnitude();
            r *= inv_radius;
            i.moment += (3.0 * i.grav_accel.magnitude() * inv_radius) * r.cross(i.j * r);
        }
        let omega_b = i.ti2b * i.omega_planet;
        if hold_down {
            self.pqrdot = ColumnVector3::zero();
            self.pqridot = self.pqrdot - i.pqri.cross(omega_b);
        } else {
            self.pqridot = i.jinv * (i.moment - i.pqri.cross(i.j * i.pqri));
            self.pqrdot = self.pqridot + i.pqri.cross(omega_b);
        }
    }

    fn calculate_uvwdot(&mut self, hold_down: bool, trim_status: bool) {
        let i = &self.input;
        self.body_accel = if hold_down && !trim_status {
            ColumnVector3::zero()
        } else {
            i.force / i.mass
        };
        self.uvwdot = self.body_accel - (i.pqr + 2.0 * (i.ti2b * i.omega_planet)).cross(i.uvw);
        self.uvwdot -= i.ti2b
            * i.omega_planet
                .cross(i.omega_planet.cross(i.inertial_position));
        if hold_down {
            self.uvwidot = i
                .omega_planet
                .cross(i.omega_planet.cross(i.inertial_position));
            self.uvwdot = ColumnVector3::zero();
        } else {
            self.uvwdot += i.tec2b * i.grav_accel;
            self.uvwidot = i.tb2i * self.body_accel + i.tec2i * i.grav_accel;
        }
    }

    /// `SetHoldDown`.
    pub fn set_hold_down(&mut self, hd: bool) {
        if hd {
            let i = &self.input;
            self.uvwidot = i
                .omega_planet
                .cross(i.omega_planet.cross(i.inertial_position));
            self.uvwdot = ColumnVector3::zero();
            self.pqrdot = ColumnVector3::zero();
            self.pqridot = self.pqrdot - i.pqri.cross(i.ti2b * i.omega_planet);
        }
    }

    /// `CalculateFrictionForces`: projected Gauss-Seidel on the friction
    /// multipliers.
    fn calculate_friction_forces(&mut self, dt: f64) {
        self.friction_forces = ColumnVector3::zero();
        self.friction_moments = ColumnVector3::zero();
        let n = self.input.multipliers.len();
        if n == 0 {
            return;
        }
        let i = &mut self.input;
        let mut a = friction_matrix(i, n);
        let mut rhs = vec![0.0; n];
        let mut vdot = self.uvwdot;
        if dt > 0. {
            vdot += (i.uvw - i.tec2b * i.terrain_velocity) / dt;
        }
        let mut wdot = self.pqrdot;
        if dt > 0. {
            wdot += (i.pqr - i.tec2b * i.terrain_angular_vel) / dt;
        }
        let rows = a.chunks_exact_mut(n).zip(&mut rhs).zip(&i.multipliers);
        for (r, ((row, rhs_r), m)) in rows.enumerate() {
            let Some(&d) = row.get(r) else {
                continue;
            };
            let (u, arm) = (m.force_jacobian, m.lever_arm);
            *rhs_r = -dot_product(u, vdot + wdot.cross(arm)) / d;
            for x in row.iter_mut() {
                *x /= d;
            }
        }
        for _ in 0..50 {
            let mut norm = 0.;
            for (r, (row, &rhs_r)) in a.chunks_exact(n).zip(&rhs).enumerate() {
                let mut dlambda = rhs_r;
                for (&arc, mc) in row.iter().zip(&i.multipliers) {
                    dlambda -= arc * mc.value;
                }
                let Some(m) = i.multipliers.get_mut(r) else {
                    break;
                };
                let lambda0 = m.value;
                m.value = constrain(m.min, lambda0 + dlambda, m.max);
                norm += (m.value - lambda0).abs();
            }
            if norm < 1E-5 {
                break;
            }
        }
        for m in &i.multipliers {
            let f = m.value * m.force_jacobian;
            self.friction_forces += f;
            self.friction_moments += m.lever_arm.cross(f);
        }
        let accel = self.friction_forces / i.mass;
        let omegadot = i.jinv * self.friction_moments;
        self.body_accel += accel;
        self.uvwdot += accel;
        self.uvwidot += i.tb2i * accel;
        self.pqrdot += omegadot;
        self.pqridot += omegadot;
    }

    /// `GetWeight(idx)`.
    #[must_use]
    pub fn weight(&self, idx: Axis) -> f64 {
        self.input.mass * (self.input.tec2b * self.input.grav_accel)[idx]
    }
}

/// The symmetric `n` x `n` (row-major) matrix of the friction
/// multipliers' couplings: the upper triangle computed row by row as
/// JSBSim does, the lower one copied from it.
fn friction_matrix(i: &Inputs, n: usize) -> Vec<f64> {
    let mut a = vec![0.0; n * n];
    for (r, (row, mr)) in a.chunks_exact_mut(n).zip(&i.multipliers).enumerate() {
        let (u, arm) = (mr.force_jacobian, mr.lever_arm);
        let v1 = u / i.mass;
        let v2 = i.jinv * arm.cross(u);
        for (x, mc) in row.iter_mut().zip(&i.multipliers).skip(r) {
            *x = dot_product(mc.force_jacobian, v1 + v2.cross(mc.lever_arm));
        }
    }
    for r in 1..n {
        for c in 0..r {
            if let Some(upper) = a.get(c * n + r).copied()
                && let Some(lower) = a.get_mut(r * n + c)
            {
                *lower = upper;
            }
        }
    }
    a
}

/// `FGAccelerations::bind`.
pub fn bind(b: &mut Binder<'_>) -> Result<(), Error> {
    for (name, i) in [("p", E_P), ("q", E_Q), ("r", E_R)] {
        b.getter(&format!("accelerations/{name}dot-rad_sec2"), move |m| {
            m.accelerations.pqrdot[i]
        })?;
    }
    for (name, i) in [("p", E_P), ("q", E_Q), ("r", E_R)] {
        b.getter(&format!("accelerations/{name}idot-rad_sec2"), move |m| {
            m.accelerations.pqridot[i]
        })?;
    }
    for (name, i) in [("u", E_U), ("v", E_V), ("w", E_W)] {
        b.getter(&format!("accelerations/{name}dot-ft_sec2"), move |m| {
            m.accelerations.uvwdot[i]
        })?;
    }
    for (name, i) in [("u", E_U), ("v", E_V), ("w", E_W)] {
        b.getter(&format!("accelerations/{name}idot-ft_sec2"), move |m| {
            m.accelerations.uvwidot[i]
        })?;
    }
    b.getter("accelerations/gravity-ft_sec2", |m| {
        m.accelerations.input.grav_accel.magnitude()
    })?;
    b.property(
        "simulation/gravitational-torque",
        Kind::Bool,
        |m| f64::from(u8::from(m.accelerations.grav_torque)),
        |m, v| m.accelerations.grav_torque = v != 0.0,
    )?;
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("forces/fb{name}-weight-lbs"), move |m| {
            m.accelerations.weight(i)
        })?;
    }
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("forces/fb{name}-total-lbs"), move |m| {
            m.accelerations.input.force[i] + m.accelerations.friction_forces[i]
        })?;
    }
    for (name, i) in [("l", E_L), ("m", E_M), ("n", E_N)] {
        b.getter(&format!("moments/{name}-total-lbsft"), move |m| {
            m.accelerations.input.moment[i] + m.accelerations.friction_moments[i]
        })?;
    }
    for (name, i) in [("l", E_L), ("m", E_M), ("n", E_N)] {
        b.getter(&format!("moments/{name}-gear-lbsft"), move |m| {
            m.accelerations.input.ground_moment[i] + m.accelerations.friction_moments[i]
        })?;
    }
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("forces/fb{name}-gear-lbs"), move |m| {
            m.accelerations.input.ground_force[i] + m.accelerations.friction_forces[i]
        })?;
    }
    Ok(())
}
