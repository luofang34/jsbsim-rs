// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2010 T. Kreitler (t.kreitler@web.de)
// Derived from JSBSim src/models/propulsion/FGRotor.cpp (hub_vel_body2ca
// to body_moments).
//
//! The rotor's blade element equations, /SH79/ eqn(19-43), in the order
//! `CalcRotorState` applies them.

use std::f64::consts::PI;

use super::{HSR_TO_TBO, Rotor, TBO_TO_HSR};
use crate::math::{ColumnVector3, E_L, E_M, E_N, E_P, E_Q, E_R, E_U, E_V, E_W};

fn sqr(x: f64) -> f64 {
    x * x
}

impl Rotor {
    /// Body to hub-shaft axes (`TboToHsr * InvTransform`).
    fn body_to_shaft(&self) -> crate::math::Matrix33 {
        TBO_TO_HSR * self.inv_transform
    }

    /// `hub_vel_body2ca`: the air velocity at the hub (the hub `hub` ft
    /// from the CG) in control axes, /SH79/ eqn(19-22); sets the control
    /// axes' orientation.
    pub(super) fn hub_vel_body2ca(
        &mut self,
        uvw: ColumnVector3,
        pqr: ColumnVector3,
        hub: ColumnVector3,
        a_ic: f64,
        b_ic: f64,
    ) -> ColumnVector3 {
        let v_r = uvw + pqr.cross(hub);
        let v_shaft = self.body_to_shaft() * v_r;
        let beta = libm::atan2(v_shaft[E_V], v_shaft[E_U]);
        self.state.beta_orient = beta;
        let (sb, cb) = libm::sincos(beta);
        ColumnVector3::new(
            v_shaft[E_U] * cb + v_shaft[E_V] * sb,
            0.0,
            v_shaft[E_W] - b_ic * v_shaft[E_U] - a_ic * v_shaft[E_V],
        )
    }

    /// `fus_angvel_body2ca`: the fuselage rates in control axes, /SH79/
    /// eqn(30,31).
    pub(super) fn fus_angvel_body2ca(&self, pqr: ColumnVector3) -> ColumnVector3 {
        let av_s = self.body_to_shaft() * pqr;
        let (sb, cb) = libm::sincos(self.state.beta_orient);
        ColumnVector3::new(
            av_s[E_P] * cb + av_s[E_Q] * sb,
            -av_s[E_P] * sb + av_s[E_Q] * cb,
            av_s[E_R],
        )
    }

    /// `calc_flow_and_thrust`: the induced inflow lagged towards its
    /// momentum value over the step, the inflow ratio and the thrust,
    /// lbs; `flow_scale` (0.5 to 1) is the ground effect, /TA77/ eqn(10a).
    pub(super) fn calc_flow_and_thrust(
        &mut self,
        theta_0: f64,
        uw: f64,
        ww: f64,
        flow_scale: f64,
    ) -> f64 {
        let c = &self.config;
        let s = &mut self.state;
        let omega_r = s.omega * c.radius;
        s.mu = uw / (s.omega * c.radius);
        if s.mu > 0.7 {
            s.mu = 0.7;
        }
        let mu2 = sqr(s.mu);
        let b = &c.b;
        let ct_t0 = (1.0 / 3.0 * b[3] + 1.0 / 2.0 * c.tip_loss_b * mu2
            - 4.0 / (9.0 * PI) * s.mu * mu2)
            * theta_0;
        let ct_t1 = (1.0 / 4.0 * b[4] + 1.0 / 4.0 * b[2] * mu2) * c.blade_twist;
        let ct_l = (1.0 / 2.0 * b[2] + 1.0 / 4.0 * mu2) * s.lambda;
        let mut c0 = (c.lift_curve_slope / 2.0) * (ct_l + ct_t0 + ct_t1) * c.solidity;
        c0 /= 2.0 * (sqr(s.mu) + sqr(s.lambda)).sqrt() + 1e-15;
        // /SH79/ eqn(26) as dnu/dt = (Ct / (2 sqrt(mu^2 + lambda^2)) - nu)
        // / tau, integrated over the step with mu and lambda held.
        s.nu = flow_scale * ((s.nu - c0) * libm::exp(-c.dt / c.inflow_lag) + c0);
        s.lambda = ww / (s.omega * c.radius) - s.nu;
        let ct_l = (1.0 / 2.0 * b[2] + 1.0 / 4.0 * mu2) * s.lambda;
        let ct_over_sigma = (c.lift_curve_slope / 2.0) * (ct_l + ct_t0 + ct_t1);
        let thrust = f64::from(c.blade_num)
            * c.blade_chord
            * c.radius
            * s.rho
            * sqr(omega_r)
            * ct_over_sigma;
        s.c_t = ct_over_sigma * c.solidity;
        s.v_induced = s.nu * omega_r;
        thrust
    }

    /// `calc_coning_angle`, /SH79/ eqn(29).
    pub(super) fn calc_coning_angle(&mut self, theta_0: f64) {
        let c = &self.config;
        let s = &mut self.state;
        let lock_gamma = c.lock_number_by_rho * s.rho;
        let mu = s.mu;
        let a0_l = (1.0 / 6.0 + 0.04 * mu * mu * mu) * s.lambda;
        let a0_t0 = (1.0 / 8.0 + 1.0 / 8.0 * mu * mu) * theta_0;
        let a0_t1 = (1.0 / 10.0 + 1.0 / 12.0 * mu * mu) * c.blade_twist;
        s.a0 = lock_gamma * (a0_l + a0_t0 + a0_t1);
    }

    /// `calc_flapping_angles`: the flapping relative to the control axes,
    /// /SH79/ eqn(32), given the fuselage rates there.
    pub(super) fn calc_flapping_angles(&mut self, theta_0: f64, pqr_fus_w: ColumnVector3) {
        let c = &self.config;
        let s = &mut self.state;
        let lock_gamma = c.lock_number_by_rho * s.rho;
        let mu2_2 = sqr(s.mu) / 2.0;
        // The usual approximation for rectangular blades.
        let t075 = theta_0 + 0.75 * c.blade_twist;
        let (p, q, omega) = (pqr_fus_w[E_P], pqr_fus_w[E_Q], s.omega);
        let advance = (2.0 * s.lambda + (8.0 / 3.0) * t075) * s.mu;
        s.a_1 = 1.0 / (1.0 - mu2_2) * (advance + p / omega - 16.0 * q / (lock_gamma * omega));
        s.b_1 = 1.0 / (1.0 + mu2_2)
            * ((4.0 / 3.0) * s.mu * s.a0 - q / omega - 16.0 * p / (lock_gamma * omega));
        s.a_dw = 1.0 / (1.0 - mu2_2)
            * (advance
                - 24.0 * q / (lock_gamma * omega) * (1.0 - (0.29 * t075 / (s.c_t / c.solidity))));
    }

    /// `calc_drag_and_side_forces`, /SH79/ eqn(38,39).
    pub(super) fn calc_drag_and_side_forces(&mut self, theta_0: f64, thrust: f64) {
        let c = &self.config;
        let s = &mut self.state;
        let t075 = theta_0 + 0.75 * c.blade_twist;
        s.h_drag = thrust * s.a_dw;
        let (a0, a_1, b_1, mu, lambda) = (s.a0, s.a_1, s.b_1, s.mu, s.lambda);
        let mut cy_over_sigma = 0.75 * b_1 * lambda - 1.5 * a0 * mu * lambda
            + 0.25 * a_1 * b_1 * mu
            - a0 * a_1 * sqr(mu)
            + (1.0 / 6.0) * a0 * a_1
            - (0.75 * mu * a0 - (1.0 / 3.0) * b_1 - 0.5 * sqr(mu) * b_1) * t075;
        cy_over_sigma *= c.lift_curve_slope / 2.0;
        s.j_side = f64::from(c.blade_num)
            * c.blade_chord
            * c.radius
            * s.rho
            * sqr(s.omega * c.radius)
            * cy_over_sigma;
    }

    /// `calc_torque`: /SH79/ eqn(36) simplified, with Bramwell's blade
    /// drag estimate (eqn(3.43), (3.44)).
    pub(super) fn calc_torque(&mut self, thrust: f64) {
        let c = &self.config;
        let s = &mut self.state;
        let delta_dr = 0.009 + 0.3 * sqr(6.0 * s.c_t / (c.lift_curve_slope * c.solidity));
        s.torque = s.rho
            * f64::from(c.blade_num)
            * c.blade_chord
            * delta_dr
            * sqr(s.omega * c.radius)
            * c.r[2]
            * (1.0 + 4.5 * sqr(s.mu))
            / 8.0
            - (thrust * s.lambda + s.h_drag * s.mu) * c.radius;
    }

    /// `calc_downwash_angles`: the downwash relative to the shaft from the
    /// airspeed alone and the last cyclic flapping; zero for a main rotor's
    /// downwash pointing down, positive towards the nose and to the left.
    pub(super) fn calc_downwash_angles(&mut self, aero_uvw: ColumnVector3) {
        let v_shaft = self.body_to_shaft() * aero_uvw;
        let s = &mut self.state;
        let vertical = s.v_induced - v_shaft[E_W];
        s.theta_downwash = libm::atan2(-v_shaft[E_U], vertical) + s.a1s;
        s.phi_downwash = libm::atan2(v_shaft[E_V], vertical) + s.b1s;
    }

    /// `body_forces`: the forces from control to shaft axes, in thruster
    /// axes, /SH79/ eqn(40,41).
    pub(super) fn body_forces(&self, a_ic: f64, b_ic: f64, thrust: f64) -> ColumnVector3 {
        let s = &self.state;
        let (sb, cb) = libm::sincos(s.beta_orient);
        let f_s = ColumnVector3::new(
            -s.h_drag * cb - s.j_side * sb + thrust * b_ic,
            -s.h_drag * sb + s.j_side * cb + thrust * a_ic,
            -thrust,
        );
        HSR_TO_TBO * f_s
    }

    /// `body_moments`: the hinge offset's flapping moments and the torque
    /// with its sense, in thruster axes; sets the flapping relative to the
    /// shaft, /SH79/ eqn(43).
    pub(super) fn body_moments(&mut self, a_ic: f64, b_ic: f64) -> ColumnVector3 {
        let c = &self.config;
        let s = &mut self.state;
        let (sb, cb) = libm::sincos(s.beta_orient);
        s.a1s = s.a_1 * cb + s.b_1 * sb - b_ic;
        s.b1s = s.b_1 * cb - s.a_1 * sb + a_ic;
        let mf =
            0.5 * c.hinge_offset * f64::from(c.blade_num) * s.omega * s.omega * c.blade_mass_moment;
        let mut m_s = ColumnVector3::zero();
        m_s[E_L] = mf * s.b1s;
        m_s[E_M] = mf * s.a1s;
        m_s[E_N] = s.torque * c.sense;
        HSR_TO_TBO * m_s
    }
}
