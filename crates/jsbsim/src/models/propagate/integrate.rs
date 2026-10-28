// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGPropagate.cpp (Integrate).
//
//! The integration schemes, with JSBSim's coefficients and evaluation
//! order.

use super::Propagate;
use crate::math::{ColumnVector3, E_P, E_Q, E_R, Quaternion, dot_product};

/// `eIntegrateType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integrator {
    /// Frozen.
    None = 0,
    /// Rectangular Euler.
    RectEuler,
    /// Trapezoidal.
    Trapezoidal,
    /// Adams-Bashforth 2nd order.
    AdamsBashforth2,
    /// Adams-Bashforth 3rd order.
    AdamsBashforth3,
    /// Adams-Bashforth 4th order.
    AdamsBashforth4,
    /// Buss 1st order (rotational position only).
    Buss1,
    /// Buss 2nd order (rotational position only).
    Buss2,
    /// Local linearization (rotational position only).
    LocalLinearization,
    /// Adams-Bashforth 5th order.
    AdamsBashforth5,
}

fn shift<T: Clone>(history: &mut [T; 5], newest: T) {
    history.rotate_right(1);
    history[0] = newest;
}

/// `Integrate` for vectors. Returns false for the rotational-only
/// schemes, which JSBSim rejects with an exception.
fn integrate_vector(
    integrand: &mut ColumnVector3,
    val: ColumnVector3,
    d: &mut [ColumnVector3; 5],
    dt: f64,
    kind: i32,
) -> bool {
    shift(d, val);
    let k = kind;
    if k == Integrator::RectEuler as i32 {
        *integrand += dt * d[0];
    } else if k == Integrator::Trapezoidal as i32 {
        *integrand += 0.5 * dt * (d[0] + d[1]);
    } else if k == Integrator::AdamsBashforth2 as i32 {
        *integrand += dt * (1.5 * d[0] - 0.5 * d[1]);
    } else if k == Integrator::AdamsBashforth3 as i32 {
        *integrand += (1.0 / 12.0) * dt * (23.0 * d[0] - 16.0 * d[1] + 5.0 * d[2]);
    } else if k == Integrator::AdamsBashforth4 as i32 {
        *integrand += (1.0 / 24.0) * dt * (55.0 * d[0] - 59.0 * d[1] + 37.0 * d[2] - 9.0 * d[3]);
    } else if k == Integrator::AdamsBashforth5 as i32 {
        *integrand += dt
            * ((1901. / 720.) * d[0] - (1387. / 360.) * d[1] + (109. / 30.) * d[2]
                - (637. / 360.) * d[3]
                + (251. / 720.) * d[4]);
    } else if k == Integrator::Buss1 as i32
        || k == Integrator::Buss2 as i32
        || k == Integrator::LocalLinearization as i32
    {
        return false;
    }
    true
}

impl Propagate {
    pub(super) fn integrate_all(&mut self, dt: f64) {
        self.integrate_attitude(dt);
        let s = &mut self.state;
        let ok = integrate_vector(
            &mut s.pqri,
            self.input.pqridot,
            &mut s.dq_pqridot,
            dt,
            self.integrator_rotational_rate,
        ) && integrate_vector(
            &mut s.inertial_position,
            s.inertial_velocity,
            &mut s.dq_inertial_velocity,
            dt,
            self.integrator_translational_position,
        ) && integrate_vector(
            &mut s.inertial_velocity,
            self.input.uvwidot,
            &mut s.dq_uvwidot,
            dt,
            self.integrator_translational_rate,
        );
        if !ok {
            tracing::error!(
                "Can only use Buss (1 & 2) or local linearization integration methods in for rotational position!"
            );
        }
    }

    /// `Integrate` for the attitude quaternion, normalised afterwards
    /// except for the Buss schemes, whose exponential is already unit.
    fn integrate_attitude(&mut self, dt: f64) {
        let s = &mut self.state;
        shift(&mut s.dq_qtrndot, s.qtrndot.clone());
        let d = &s.dq_qtrndot;
        let k = self.integrator_rotational_position;
        let step = |q: Quaternion| -> Quaternion { q };
        let increment = if k == Integrator::RectEuler as i32 {
            Some(dt * &d[0])
        } else if k == Integrator::Trapezoidal as i32 {
            Some(0.5 * dt * &(&d[0] + &d[1]))
        } else if k == Integrator::AdamsBashforth2 as i32 {
            Some(dt * &(&(1.5 * &d[0]) - &(0.5 * &d[1])))
        } else if k == Integrator::AdamsBashforth3 as i32 {
            let sum = &(&(23.0 * &d[0]) - &(16.0 * &d[1])) + &(5.0 * &d[2]);
            Some((1.0 / 12.0) * dt * &sum)
        } else if k == Integrator::AdamsBashforth4 as i32 {
            let sum = &(&(&(55.0 * &d[0]) - &(59.0 * &d[1])) + &(37.0 * &d[2])) - &(9.0 * &d[3]);
            Some((1.0 / 24.0) * dt * &sum)
        } else if k == Integrator::AdamsBashforth5 as i32 {
            let sum = &(&(&(&((1901. / 720.) * &d[0]) - &((1387. / 360.) * &d[1]))
                + &((109. / 30.) * &d[2]))
                - &((637. / 360.) * &d[3]))
                + &((251. / 720.) * &d[4]);
            Some(dt * &sum)
        } else {
            None
        };
        if let Some(inc) = increment {
            s.q_attitude_eci += &step(inc);
        } else if k == Integrator::Buss1 as i32 {
            s.q_attitude_eci = &s.q_attitude_eci * &Quaternion::exp(0.5 * dt * s.pqri);
            return;
        } else if k == Integrator::Buss2 as i32 {
            let wi = s.pqri;
            let wdoti = self.input.pqridot;
            let omega = wi + 0.5 * dt * wdoti + (dt * dt / 12. * wdoti).cross(wi);
            s.q_attitude_eci = &s.q_attitude_eci * &Quaternion::exp(0.5 * dt * omega);
            return;
        } else if k == Integrator::LocalLinearization as i32 {
            s.q_attitude_eci =
                &s.q_attitude_eci * &local_linearization(s.pqri, self.input.pqridot, dt);
        }
        s.q_attitude_eci.normalize();
    }
}

fn local_linearization(pqri: ColumnVector3, pqridot: ColumnVector3, dt: f64) -> Quaternion {
    let wi = 0.5 * pqri;
    let wdoti = 0.5 * pqridot;
    let omegak2 = dot_product(pqri, pqri);
    let omegak = if omegak2 > 1E-6 { omegak2.sqrt() } else { 1E-6 };
    let rhok = 0.5 * dt * omegak;
    let c1 = libm::cos(rhok);
    let c2 = 2.0 * libm::sin(rhok) / omegak;
    let c3 = 4.0 * (1.0 - c1) / (omegak * omegak);
    let c4 = 4.0 * (dt - c2) / (omegak * omegak);
    let omega = c2 * wi + c3 * wdoti + (c4 * wi).cross(wdoti);
    Quaternion::from_components(
        c1 - c4 * dot_product(wi, wdoti),
        omega[E_P],
        omega[E_Q],
        omega[E_R],
    )
}
