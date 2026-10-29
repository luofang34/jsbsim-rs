// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGInitialCondition.cpp (wind
// setters and getters).
//
//! Wind. Every wind setter keeps the ground velocity and changes the true
//! airspeed, alpha and beta.

use super::InitialCondition;
use crate::math::{Axis, ColumnVector3, E_PSI, E_U, E_V, E_W, dot_product};
use crate::units::{DEGTORAD, KTSTOFPS, RADTODEG};

impl InitialCondition {
    /// The airspeed is the ground velocity plus `wind_ned`.
    fn apply_wind_ned(&mut self, wind_ned: ColumnVector3) {
        let vt_ned = self.uvw_ned + wind_ned;
        self.vt = vt_ned.magnitude();
        self.calc_aero_angles(vt_ned);
    }

    /// `SetWindNEDFpsIC`.
    pub fn set_wind_ned_fps(&mut self, w_n: f64, w_e: f64, w_d: f64) {
        self.apply_wind_ned(ColumnVector3::new(w_n, w_e, w_d));
    }

    /// Replaces the wind component along the horizontal unit vector `dir`
    /// with `speed_kts` (Gram-Schmidt).
    fn set_wind_along_kts(&mut self, dir: ColumnVector3, speed_kts: f64) {
        let mut wind_ned = self.vt_ned() - self.uvw_ned;
        wind_ned -= dot_product(wind_ned, dir) * dir;
        wind_ned += (speed_kts * KTSTOFPS) * dir;
        self.apply_wind_ned(wind_ned);
    }

    /// `SetCrossWindKtsIC`: wind across the heading, positive from left to
    /// right.
    pub fn set_cross_wind_kts(&mut self, cross: f64) {
        let dir = ColumnVector3::new(
            -self.orientation.sin_euler(E_PSI),
            self.orientation.cos_euler(E_PSI),
            0.,
        );
        self.set_wind_along_kts(dir, cross);
    }

    /// `SetHeadWindKtsIC`: wind against the heading.
    pub fn set_head_wind_kts(&mut self, head: f64) {
        let dir = ColumnVector3::new(
            -self.orientation.cos_euler(E_PSI),
            -self.orientation.sin_euler(E_PSI),
            0.,
        );
        self.set_wind_along_kts(dir, head);
    }

    /// `SetWindDownKtsIC`: the downward airspeed becomes the downward
    /// ground speed plus `w_d`; the horizontal airspeed is kept.
    pub fn set_wind_down_kts(&mut self, w_d: f64) {
        let mut vt_ned = self.vt_ned();
        vt_ned[E_W] = self.uvw_ned[E_W] + w_d * KTSTOFPS;
        self.vt = vt_ned.magnitude();
        self.calc_aero_angles(vt_ned);
    }

    /// `SetWindMagKtsIC`: the horizontal wind speed, keeping its direction
    /// (north when there is next to no wind) and its vertical component.
    pub fn set_wind_mag_kts(&mut self, mag: f64) {
        let mut wind_ned = self.vt_ned() - self.uvw_ned;
        let mut head = ColumnVector3::new(wind_ned[E_U], wind_ned[E_V], 0.);
        let wind_mag = head.magnitude();
        if wind_mag > 0.001 {
            head *= (mag * KTSTOFPS) / wind_mag;
        } else {
            head = ColumnVector3::new(mag * KTSTOFPS, 0., 0.);
        }
        wind_ned[E_U] = head[E_U];
        wind_ned[E_V] = head[E_V];
        self.apply_wind_ned(wind_ned);
    }

    /// `SetWindMagFpsIC`.
    pub fn set_wind_mag_fps(&mut self, mag: f64) {
        self.set_wind_mag_kts(mag * crate::units::FPSTOKTS);
    }

    /// `SetWindDirDegIC`: the direction the wind blows toward, keeping its
    /// horizontal speed and vertical component.
    pub fn set_wind_dir_deg(&mut self, dir: f64) {
        let mut wind_ned = self.vt_ned() - self.uvw_ned;
        let mag = wind_ned.magnitude2(E_U, E_V);
        let (sin_dir, cos_dir) = libm::sincos(dir * DEGTORAD);
        wind_ned[E_U] = mag * cos_dir;
        wind_ned[E_V] = mag * sin_dir;
        self.apply_wind_ned(wind_ned);
    }

    /// `GetWindNEDFpsIC`.
    #[must_use]
    pub fn wind_ned_fps(&self) -> ColumnVector3 {
        self.vt_ned() - self.uvw_ned
    }

    /// `GetWindMagFpsIC`: horizontal wind speed.
    #[must_use]
    pub fn wind_mag_fps(&self) -> f64 {
        self.wind_ned_fps().magnitude2(E_U, E_V)
    }

    /// `GetWindDirDegIC`; 0 without horizontal wind.
    #[must_use]
    pub fn wind_dir_deg(&self) -> f64 {
        let wind_ned = self.wind_ned_fps();
        if wind_ned.magnitude2(E_U, E_V) == 0.0 {
            0.0
        } else {
            libm::atan2(wind_ned[E_V], wind_ned[E_U]) * RADTODEG
        }
    }

    /// `GetWindUFpsIC`, `GetWindVFpsIC`, `GetWindWFpsIC` by axis.
    #[must_use]
    pub fn body_wind_fps(&self, idx: Axis) -> f64 {
        let vt_body = self.tw2b * ColumnVector3::new(self.vt, 0., 0.);
        let uvw_body = self.orientation.t() * self.uvw_ned;
        (vt_body - uvw_body)[idx]
    }
}
