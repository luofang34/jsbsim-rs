// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGInitialCondition.cpp (speed
// setters and getters).
//
//! Speeds. Calibrated, equivalent and Mach speeds become a true airspeed at
//! the current altitude; the true airspeed keeps its direction, so with wind
//! the ground velocity may turn. Body and local velocity components change
//! the ground velocity and keep the wind.

use super::{Environment, InitialCondition, SpeedSet};
use crate::math::{Axis, ColumnVector3, E_PSI, E_X, E_Y, E_Z};
use crate::models::atmosphere::STD_DAY_SL_DENSITY;
use crate::models::auxiliary::{mach_from_vcalibrated, vcalibrated_from_mach};
use crate::units::{FPSTOKTS, KTSTOFPS};

impl InitialCondition {
    /// `SetVequivalentKtsIC`.
    pub fn set_vequivalent_kts(&mut self, env: Environment<'_>, ve: f64) {
        let altitude_asl = self.altitude_asl_ft();
        let rho = env.atmosphere.get_density(altitude_asl);
        self.set_vtrue_fps(ve * KTSTOFPS * (STD_DAY_SL_DENSITY / rho).sqrt());
        self.last_speed_set = SpeedSet::Ve;
    }

    /// `SetMachIC`.
    pub fn set_mach(&mut self, env: Environment<'_>, mach: f64) {
        let altitude_asl = self.altitude_asl_ft();
        let sound_speed = env.atmosphere.get_sound_speed(altitude_asl);
        self.set_vtrue_fps(mach * sound_speed);
        self.last_speed_set = SpeedSet::Mach;
    }

    /// `SetVcalibratedKtsIC`; the sign of `vcas` is ignored.
    pub fn set_vcalibrated_kts(&mut self, env: Environment<'_>, vcas: f64) {
        let altitude_asl = self.altitude_asl_ft();
        let pressure = env.atmosphere.get_pressure(altitude_asl);
        let mach = mach_from_vcalibrated(
            vcas.abs() * KTSTOFPS,
            pressure,
            env.atmosphere.std_day_sl_soundspeed,
        );
        let sound_speed = env.atmosphere.get_sound_speed(altitude_asl);
        self.set_vtrue_fps(mach * sound_speed);
        self.last_speed_set = SpeedSet::Vc;
    }

    /// `SetVgroundFpsIC`: horizontal ground speed along the heading; the
    /// vertical ground speed becomes zero and the wind is kept.
    pub fn set_vground_fps(&mut self, vg: f64) {
        let vt_ned = self.vt_ned();
        let wind_ned = vt_ned - self.uvw_ned;
        self.uvw_ned[E_X] = vg * self.orientation.cos_euler(E_PSI);
        self.uvw_ned[E_Y] = vg * self.orientation.sin_euler(E_PSI);
        self.uvw_ned[E_Z] = 0.;
        let vt_ned = self.uvw_ned + wind_ned;
        self.vt = vt_ned.magnitude();
        self.calc_aero_angles(vt_ned);
        self.last_speed_set = SpeedSet::Vg;
    }

    /// `SetVgroundKtsIC`.
    pub fn set_vground_kts(&mut self, vg: f64) {
        self.set_vground_fps(vg * KTSTOFPS);
    }

    /// `SetVtrueFpsIC`: the airspeed magnitude changes, its direction and
    /// the wind are kept. Below 0.1 ft/s the direction is the body x axis
    /// rotated by alpha and beta.
    pub fn set_vtrue_fps(&mut self, vtrue: f64) {
        let mut vt_ned = self.vt_ned();
        let wind_ned = vt_ned - self.uvw_ned;
        if self.vt > 0.1 {
            vt_ned *= vtrue / self.vt;
        } else {
            vt_ned = self.orientation.t_inv() * self.tw2b * ColumnVector3::new(vtrue, 0., 0.);
        }
        self.vt = vtrue;
        self.uvw_ned = vt_ned - wind_ned;
        self.calc_aero_angles(vt_ned);
        self.last_speed_set = SpeedSet::Vt;
    }

    /// `SetVtrueKtsIC`.
    pub fn set_vtrue_kts(&mut self, vtrue: f64) {
        self.set_vtrue_fps(vtrue * KTSTOFPS);
    }

    /// `SetBodyVelFpsIC(idx, vel)`: one body component of the ground
    /// velocity (`E_U`, `E_V` or `E_W`); the wind is kept.
    pub fn set_body_vel_fps(&mut self, idx: Axis, vel: f64) {
        let tb2l = self.orientation.t_inv();
        let vt_ned = self.vt_ned();
        let mut uvw_body = self.orientation.t() * self.uvw_ned;
        let wind_ned = vt_ned - self.uvw_ned;
        uvw_body[idx] = vel;
        self.uvw_ned = tb2l * uvw_body;
        let vt_ned = self.uvw_ned + wind_ned;
        self.vt = vt_ned.magnitude();
        self.calc_aero_angles(vt_ned);
        self.last_speed_set = SpeedSet::Uvw;
    }

    /// `SetNEDVelFpsIC(idx, vel)`: one local component of the ground
    /// velocity (`E_U`, `E_V` or `E_W` for north, east, down); the wind is
    /// kept.
    pub fn set_ned_vel_fps(&mut self, idx: Axis, vel: f64) {
        let vt_ned = self.vt_ned();
        let wind_ned = vt_ned - self.uvw_ned;
        self.uvw_ned[idx] = vel;
        let vt_ned = self.uvw_ned + wind_ned;
        self.vt = vt_ned.magnitude();
        self.calc_aero_angles(vt_ned);
        self.last_speed_set = SpeedSet::Ned;
    }

    /// `GetVcalibratedKtsIC`.
    #[must_use]
    pub fn vcalibrated_kts(&self, env: Environment<'_>) -> f64 {
        let altitude_asl = self.altitude_asl_ft();
        let pressure = env.atmosphere.get_pressure(altitude_asl);
        let sound_speed = env.atmosphere.get_sound_speed(altitude_asl);
        let mach = self.vt / sound_speed;
        FPSTOKTS * vcalibrated_from_mach(mach, pressure, env.atmosphere.std_day_sl_soundspeed)
    }

    /// `GetVequivalentKtsIC`.
    #[must_use]
    pub fn vequivalent_kts(&self, env: Environment<'_>) -> f64 {
        let altitude_asl = self.altitude_asl_ft();
        let rho = env.atmosphere.get_density(altitude_asl);
        FPSTOKTS * self.vt * (rho / STD_DAY_SL_DENSITY).sqrt()
    }

    /// `GetMachIC`.
    #[must_use]
    pub fn mach(&self, env: Environment<'_>) -> f64 {
        let altitude_asl = self.altitude_asl_ft();
        self.vt / env.atmosphere.get_sound_speed(altitude_asl)
    }
}
