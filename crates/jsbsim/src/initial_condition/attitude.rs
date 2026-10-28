// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGInitialCondition.cpp (climb rate,
// flight path, alpha, beta and Euler angle setters).
//
//! Flight path and attitude, for a steady, zero pitch rate condition.
//! Setting the climb rate or alpha keeps the airspeed magnitude and the
//! heading and solves for theta (and beta when rolled); setting beta keeps
//! the airspeed vector and alpha and solves for psi and theta; setting an
//! Euler angle keeps the body velocity unless the local or ground velocity
//! was set last.

use super::{InitialCondition, SpeedSet, wind_to_body};
use crate::math::{
    Axis, ColumnVector3, E_PHI, E_PSI, E_THT, E_U, E_V, E_W, E_X, E_Y, E_Z, Matrix33, Quaternion,
    dot_product,
};
use crate::units::DEGTORAD;

impl InitialCondition {
    /// `SetClimbRateFpsIC`: the vertical airspeed becomes `hdot`, the
    /// horizontal airspeed scales to keep its magnitude. Refused (and
    /// logged) when `hdot` exceeds the airspeed.
    pub fn set_climb_rate_fps(&mut self, hdot: f64) {
        if hdot.abs() > self.vt {
            tracing::error!("The climb rate cannot be higher than the true speed.");
            return;
        }
        let mut vt_ned = self.vt_ned();
        let wind_ned = vt_ned - self.uvw_ned;
        let hdot0 = -vt_ned[E_W];
        if hdot0.abs() < self.vt {
            let vt2 = self.vt * self.vt;
            let scale = ((vt2 - hdot * hdot) / (vt2 - hdot0 * hdot0)).sqrt();
            vt_ned[E_U] *= scale;
            vt_ned[E_V] *= scale;
        }
        vt_ned[E_W] = -hdot;
        self.uvw_ned = vt_ned - wind_ned;
        self.calc_theta_beta(self.alpha, vt_ned);
    }

    /// `SetClimbRateFpmIC`.
    pub fn set_climb_rate_fpm(&mut self, roc: f64) {
        self.set_climb_rate_fps(roc / 60.0);
    }

    /// `SetFlightPathAngleRadIC`.
    pub fn set_flight_path_angle_rad(&mut self, gamma: f64) {
        self.set_climb_rate_fps(self.vt * libm::sin(gamma));
    }

    /// `SetFlightPathAngleDegIC`.
    pub fn set_flight_path_angle_deg(&mut self, gamma: f64) {
        self.set_climb_rate_fps(self.vt * libm::sin(gamma * DEGTORAD));
    }

    /// `SetAlphaRadIC`.
    pub fn set_alpha_rad(&mut self, alpha: f64) {
        let vt_ned = self.vt_ned();
        self.calc_theta_beta(alpha, vt_ned);
    }

    /// `SetAlphaDegIC`.
    pub fn set_alpha_deg(&mut self, alpha: f64) {
        self.set_alpha_rad(alpha * DEGTORAD);
    }

    /// `calcThetaBeta`: the theta that gives angle of attack `alfa` for
    /// the airspeed `vt_ned`, then beta. Refused (and logged) when no theta
    /// gives `alfa` at the current roll angle.
    fn calc_theta_beta(&mut self, alfa: f64, vt_ned: ColumnVector3) {
        let mut orient = self.orientation.euler();
        let (calpha, salpha) = (libm::cos(alfa), libm::sin(alfa));
        let o = &self.orientation;
        let (cpsi, spsi) = (o.cos_euler(E_PSI), o.sin_euler(E_PSI));
        let (cphi, sphi) = (o.cos_euler(E_PHI), o.sin_euler(E_PHI));
        let tpsi = Matrix33::new(cpsi, spsi, 0., -spsi, cpsi, 0., 0., 0., 1.);
        let tphi = Matrix33::new(1., 0., 0., 0., cphi, sphi, 0., -sphi, cphi);
        let talpha = Matrix33::new(calpha, 0., salpha, 0., 1., 0., -salpha, 0., calpha);

        let v0 = tpsi * vt_ned;
        let n = (talpha * tphi).transposed() * ColumnVector3::new(0., 0., 1.);
        let y = ColumnVector3::new(0., 1., 0.);
        let mut u = y - dot_product(y, n) * n;
        let mut p = y.cross(n);
        if dot_product(p, v0) < 0.0 {
            p *= -1.0;
        }
        p.normalize();
        u *= dot_product(v0, y) / dot_product(u, y);

        // No theta exists when the roll angle is too large for this alpha.
        if dot_product(v0, v0) < dot_product(u, u) {
            tracing::error!("Cannot modify angle 'alpha' from {} to {alfa}", self.alpha);
            return;
        }
        let v1 = u + (dot_product(v0, v0) - dot_product(u, u)).sqrt() * p;
        let mut v0xz = ColumnVector3::new(v0[E_U], 0., v0[E_W]);
        let mut v1xz = ColumnVector3::new(v1[E_U], 0., v1[E_W]);
        v0xz.normalize();
        v1xz.normalize();
        let sin_theta = v1xz.cross(v0xz)[E_Y];
        orient[E_THT] = libm::asin(sin_theta);
        self.orientation = Quaternion::from_euler_vector(orient);

        let v2 = talpha * self.orientation.t() * vt_ned;
        self.alpha = alfa;
        self.beta = libm::atan2(v2[E_V], v2[E_U]);
        let (mut cbeta, mut sbeta) = (1.0, 0.0);
        if self.vt != 0.0 {
            cbeta = v2[E_U] / self.vt;
            sbeta = v2[E_V] / self.vt;
        }
        self.tw2b = wind_to_body(calpha, salpha, cbeta, sbeta);
        self.tb2w = self.tw2b.transposed();
    }

    /// `SetBetaRadIC`: keeps the airspeed vector and alpha, so psi and
    /// theta change (psi especially with cross wind).
    pub fn set_beta_rad(&mut self, bta: f64) {
        let vt_ned = self.vt_ned();
        let mut orient = self.orientation.euler();
        self.beta = bta;
        let (calpha, salpha) = (libm::cos(self.alpha), libm::sin(self.alpha));
        let (cbeta, sbeta) = (libm::cos(self.beta), libm::sin(self.beta));
        let o = &self.orientation;
        let (cphi, sphi) = (o.cos_euler(E_PHI), o.sin_euler(E_PHI));
        let tphi_inv = Matrix33::new(1., 0., 0., 0., cphi, -sphi, 0., sphi, cphi);
        self.tw2b = wind_to_body(calpha, salpha, cbeta, sbeta);
        self.tb2w = self.tw2b.transposed();

        let vf = tphi_inv * self.tw2b * ColumnVector3::new(self.vt, 0., 0.);
        let mut v0xy = ColumnVector3::new(vt_ned[E_X], vt_ned[E_Y], 0.);
        let mut v1xy = ColumnVector3::new(
            (v0xy[E_X] * v0xy[E_X] + v0xy[E_Y] * v0xy[E_Y] - vf[E_Y] * vf[E_Y]).sqrt(),
            vf[E_Y],
            0.,
        );
        v0xy.normalize();
        v1xy.normalize();
        if vf[E_X] < 0. {
            v0xy[E_X] *= -1.0;
        }
        let sin_psi = v1xy.cross(v0xy)[E_Z];
        let cos_psi = dot_product(v0xy, v1xy);
        orient[E_PSI] = libm::atan2(sin_psi, cos_psi);
        let tpsi = Matrix33::new(cos_psi, sin_psi, 0., -sin_psi, cos_psi, 0., 0., 0., 1.);

        let mut v2xz = tpsi * vt_ned;
        let mut vfxz = vf;
        v2xz[E_V] = 0.0;
        vfxz[E_V] = 0.0;
        v2xz.normalize();
        vfxz.normalize();
        let sin_theta = v2xz.cross(vfxz)[E_Y];
        orient[E_THT] = -libm::asin(sin_theta);
        self.orientation = Quaternion::from_euler_vector(orient);
    }

    /// `SetBetaDegIC`.
    pub fn set_beta_deg(&mut self, beta: f64) {
        self.set_beta_rad(beta * DEGTORAD);
    }

    /// `SetEulerAngleRadIC(idx, angle)` (`E_PHI`, `E_THT` or `E_PSI`). The
    /// body velocity is kept unless the local or ground velocity was set
    /// last, in which case those are kept.
    pub fn set_euler_angle_rad(&mut self, idx: Axis, angle: f64) {
        let mut vt_ned = self.vt_ned();
        let wind_ned = vt_ned - self.uvw_ned;
        let uvw_body = self.orientation.t() * self.uvw_ned;
        let mut orient = self.orientation.euler();
        orient[idx] = angle;
        self.orientation = Quaternion::from_euler_vector(orient);
        if self.last_speed_set != SpeedSet::Ned && self.last_speed_set != SpeedSet::Vg {
            self.uvw_ned = self.orientation.t_inv() * uvw_body;
            vt_ned = self.uvw_ned + wind_ned;
            self.vt = vt_ned.magnitude();
        }
        self.calc_aero_angles(vt_ned);
    }

    /// `SetPhiRadIC`, `SetThetaRadIC`, `SetPsiRadIC` in degrees.
    pub fn set_euler_angle_deg(&mut self, idx: Axis, angle: f64) {
        self.set_euler_angle_rad(idx, angle * DEGTORAD);
    }
}
