// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGTrim.cpp (setupPullup, setupTurn, updateRates).
//
//! The body rates of the pullup and turn trims.

use super::{Trim, TrimMode};
use crate::fdm::Fdm;
use crate::math::{E_P, E_PHI, E_Q, E_R, E_THT, E_U};

impl Trim {
    /// `setupPullup`: the pitch rate of the target load factor.
    pub(super) fn setup_pullup(&mut self, fdm: &Fdm) {
        let g = fdm.models.inertial.grav_accel.magnitude();
        let cgamma = libm::cos(self.fgic.flight_path_angle_rad());
        tracing::info!(
            "setPitchRateInPullup():  {g}, {cgamma}, {}",
            self.fgic.vtrue_fps()
        );
        let q = g * (self.target_nlf - cgamma) / self.fgic.vtrue_fps();
        tracing::info!("{}, {q}", self.target_nlf);
        self.fgic.set_pqr_radps(E_Q, q);
        tracing::info!("setPitchRateInPullup() complete");
    }

    /// `setupTurn`: the load factor and turn rate of the initial bank.
    pub(super) fn setup_turn(&mut self, fdm: &Fdm) {
        let phi = self.fgic.euler_rad(E_PHI);
        if phi.abs() > 0.001 && phi.abs() < 1.56 {
            self.target_nlf = 1.0 / libm::cos(phi);
            let g = fdm.models.inertial.grav_accel.magnitude();
            self.psidot = g * libm::tan(phi) / self.fgic.body_vel_fps(E_U);
            tracing::info!("{}, {}", self.target_nlf, self.psidot);
        }
    }

    /// `updateRates`: the body rates of the turn, or the pitch rate of the
    /// pullup.
    pub(super) fn update_rates_of(&mut self, fdm: &Fdm) {
        let vtrue = self.fgic.vtrue_fps();
        let g = || fdm.models.inertial.grav_accel.magnitude();
        if self.mode == TrimMode::Turn {
            let phi = self.fgic.euler_rad(E_PHI);
            let (mut p, mut q, mut r) = (0.0, 0.0, 0.0);
            if phi.abs() > 0.001 && phi.abs() < 1.56 {
                let theta = self.fgic.euler_rad(E_THT);
                self.psidot = g() * libm::tan(phi) / vtrue;
                let (sin_theta, cos_theta) = libm::sincos(theta);
                let (sin_phi, cos_phi) = libm::sincos(phi);
                p = -self.psidot * sin_theta;
                q = self.psidot * cos_theta * sin_phi;
                r = self.psidot * cos_theta * cos_phi;
            }
            self.fgic.set_pqr_radps(E_P, p);
            self.fgic.set_pqr_radps(E_Q, q);
            self.fgic.set_pqr_radps(E_R, r);
        } else if self.mode == TrimMode::Pullup && (self.target_nlf - 1.0).abs() > 0.01 {
            let cgamma = libm::cos(self.fgic.flight_path_angle_rad());
            let q = g() * (self.target_nlf - cgamma) / vtrue;
            self.fgic.set_pqr_radps(E_Q, q);
        }
    }
}
