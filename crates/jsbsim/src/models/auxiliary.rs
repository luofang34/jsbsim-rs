// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGAuxiliary.{h,cpp} (Tony Peden, Jon Berndt).
//
//! Derived flight quantities (`FGAuxiliary`): airspeeds, Mach numbers,
//! dynamic pressures, aerodynamic angles and their rates, the wind axes,
//! flight path angles, pilot-sensed accelerations, load factors and
//! position helpers.

mod airspeed;
mod bind;
mod inputs;
mod position;

#[cfg(test)]
mod tests;

pub use airspeed::{
    mach_from_impact_pressure, mach_from_vcalibrated, pitot_total_pressure, vcalibrated_from_mach,
};
pub use bind::bind;
pub use inputs::{Air, Constants, Inputs};

use super::atmosphere::{STD_DAY_SL_DENSITY, STD_DAY_SL_PRESSURE, STD_DAY_SL_TEMPERATURE};
use super::base::ModelBase;
use crate::math::{
    ColumnVector3, E_DOWN, E_EAST, E_LIFT, E_NORTH, E_P, E_PHI, E_PSI, E_Q, E_R, E_THT, E_U, E_V,
    E_W, E_X, E_Y, E_Z, Location, Matrix33,
};
use crate::units::{SLUGTOLB, rankine_to_celsius};

/// Airspeeds, Mach numbers and total conditions.
#[derive(Debug, Clone, Copy)]
pub struct Airspeeds {
    /// Calibrated airspeed, ft/s.
    pub vcas: f64,
    /// Equivalent airspeed, ft/s.
    pub veas: f64,
    /// Ground speed, ft/s.
    pub vground: f64,
    /// Mach number.
    pub mach: f64,
    /// Mach number along the body x axis.
    pub mach_u: f64,
    /// Aero-relative body velocity in Mach.
    pub mach_uvw: ColumnVector3,
    /// Dynamic pressure of the u-w components, psf.
    pub qbar_uw: f64,
    /// Dynamic pressure of the u-v components, psf.
    pub qbar_uv: f64,
    /// Reynolds number over the chord.
    pub re: f64,
    /// Pitot total pressure, psf.
    pub pt: f64,
    /// Total air temperature, °R.
    pub tat: f64,
    /// Total air temperature, °C.
    pub tatc: f64,
}

impl Default for Airspeeds {
    /// `FGAuxiliary()`: total pressure and temperature start at the
    /// standard day's sea-level values.
    fn default() -> Self {
        Self {
            vcas: 0.0,
            veas: 0.0,
            vground: 0.0,
            mach: 0.0,
            mach_u: 0.0,
            mach_uvw: ColumnVector3::zero(),
            qbar_uw: 0.0,
            qbar_uv: 0.0,
            re: 0.0,
            pt: STD_DAY_SL_PRESSURE,
            tat: STD_DAY_SL_TEMPERATURE,
            tatc: rankine_to_celsius(STD_DAY_SL_TEMPERATURE),
        }
    }
}

/// CG load factors and pilot-sensed accelerations.
#[derive(Debug, Clone, Copy, Default)]
pub struct Loads {
    /// Acceleration at the eye point, body frame, ft/s^2.
    pub pilot_accel: ColumnVector3,
    /// The same in g.
    pub pilot_accel_n: ColumnVector3,
    /// CG acceleration, body frame, g.
    pub ncg: ColumnVector3,
    /// CG acceleration in wind axes, g, with z as `1 - n`.
    pub nwcg: ColumnVector3,
    /// Longitudinal CG load factor, g.
    pub nx: f64,
    /// Lateral CG load factor, g.
    pub ny: f64,
    /// Normal CG load factor (along -z body), g.
    pub nz: f64,
}

/// Where the flight started, for the positions relative to it.
#[derive(Debug, Clone, Default)]
pub struct Start {
    /// Origin of the NEU frame: the initial position at zero height.
    pub neu_location: Location,
}

/// Derived quantities.
#[derive(Debug, Clone, Default)]
pub struct Auxiliary {
    /// Scheduling.
    pub base: ModelBase,
    /// Inputs, loaded by the executive before each run.
    pub input: Inputs,
    /// Angle of attack, rad.
    pub alpha: f64,
    /// Sideslip, rad.
    pub beta: f64,
    /// Angle of attack rate, rad/s.
    pub adot: f64,
    /// Sideslip rate, rad/s.
    pub bdot: f64,
    /// Dynamic pressure, psf.
    pub qbar: f64,
    /// True airspeed, ft/s.
    pub vt: f64,
    /// Body to wind.
    pub tb2w: Matrix33,
    /// Wind to body.
    pub tw2b: Matrix33,
    /// Aero-relative body velocity, ft/s.
    pub aero_uvw: ColumnVector3,
    /// Aero-relative body rates, rad/s.
    pub aero_pqr: ColumnVector3,
    /// Euler angle rates (phi, theta, psi), rad/s.
    pub euler_rates: ColumnVector3,
    /// Ground track, rad, in [0, 2 pi).
    pub psigt: f64,
    /// Flight path angle, rad.
    pub gamma: f64,
    /// Height over span at the CG.
    pub hoverbcg: f64,
    /// Height over span at the aerodynamic reference point.
    pub hoverbmac: f64,
    /// Visual reference point position.
    pub location_vrp: Location,
    /// Airspeeds and total conditions.
    pub airspeed: Airspeeds,
    /// Load factors and accelerations.
    pub loads: Loads,
    /// The start of the flight.
    pub start: Start,
}

impl Auxiliary {
    /// `InitModel`. Like JSBSim, `nx`, the CG and wind-axis load factors,
    /// the wind matrices and the VRP keep their values.
    pub fn init_model(&mut self) {
        self.base.init();
        let air = &self.input.air;
        self.airspeed = Airspeeds {
            pt: air.pressure,
            tat: air.temperature,
            tatc: rankine_to_celsius(air.temperature),
            ..Airspeeds::default()
        };
        self.qbar = 0.0;
        self.alpha = 0.0;
        self.beta = 0.0;
        self.adot = 0.0;
        self.bdot = 0.0;
        self.gamma = 0.0;
        self.vt = 0.0;
        self.psigt = 0.0;
        self.hoverbmac = 0.0;
        self.hoverbcg = 0.0;
        let l = &mut self.loads;
        l.nz = 0.0;
        l.ny = 0.0;
        l.pilot_accel = ColumnVector3::zero();
        l.pilot_accel_n = ColumnVector3::zero();
        self.aero_uvw = ColumnVector3::zero();
        self.aero_pqr = ColumnVector3::zero();
        self.euler_rates = ColumnVector3::zero();
    }

    /// `Run`.
    pub fn run(&mut self, holding: bool) {
        if self.base.skip() || holding {
            return;
        }
        self.update_rates();
        self.update_aero_angles();
        self.update_wind_matrices();
        self.update_airspeeds();
        self.update_accelerations();
        self.update_reference_points();
    }

    /// The Euler angle rates; at theta = ±90° psi and phi rates keep
    /// their previous values, as in JSBSim.
    fn update_rates(&mut self) {
        let i = &self.input;
        self.euler_rates[E_THT] = i.pqr[E_Q] * i.cos_phi - i.pqr[E_R] * i.sin_phi;
        if i.cos_tht != 0.0 {
            self.euler_rates[E_PSI] = (i.pqr[E_Q] * i.sin_phi + i.pqr[E_R] * i.cos_phi) / i.cos_tht;
            self.euler_rates[E_PHI] = i.pqr[E_P] + self.euler_rates[E_PSI] * i.sin_tht;
        }
        self.aero_pqr = i.pqr - i.turb_pqr;
        self.aero_uvw = i.uvw - i.tl2b * i.total_wind_ned;
    }

    /// Alpha, beta, their rates, true airspeed and dynamic pressures.
    fn update_aero_angles(&mut self) {
        let i = &self.input;
        let a = self.aero_uvw;
        self.alpha = 0.0;
        self.beta = 0.0;
        self.adot = 0.0;
        self.bdot = 0.0;
        let aero_u2 = a[E_U] * a[E_U];
        let aero_v2 = a[E_V] * a[E_V];
        let aero_w2 = a[E_W] * a[E_W];
        let m_uw = aero_u2 + aero_w2;
        let vt2 = m_uw + aero_v2;
        self.vt = vt2.sqrt();

        if self.vt > 0.001 {
            self.beta = libm::atan2(a[E_V], m_uw.sqrt());
            if m_uw >= 1E-6 {
                self.alpha = libm::atan2(a[E_W], a[E_U]);
                let vtdot =
                    (a[E_U] * i.uvwdot[E_U] + a[E_V] * i.uvwdot[E_V] + a[E_W] * i.uvwdot[E_W])
                        / self.vt;
                self.adot = (a[E_U] * i.uvwdot[E_W] - a[E_W] * i.uvwdot[E_U]) / m_uw;
                self.bdot = (i.uvwdot[E_V] * self.vt - a[E_V] * vtdot) / (self.vt * m_uw.sqrt());
            }
        }

        let density_d2 = 0.5 * i.air.density;
        self.qbar = density_d2 * vt2;
        self.airspeed.qbar_uw = density_d2 * m_uw;
        self.airspeed.qbar_uv = density_d2 * (aero_u2 + aero_v2);
    }

    /// `UpdateWindMatrices`: Stevens and Lewis' wind-to-body transform
    /// and its transpose.
    fn update_wind_matrices(&mut self) {
        let (sa, ca) = libm::sincos(self.alpha);
        let (sb, cb) = libm::sincos(self.beta);
        self.tw2b = Matrix33::new(ca * cb, -ca * sb, -sa, sb, cb, 0.0, sa * cb, -sa * sb, ca);
        self.tb2w = self.tw2b.transposed();
    }

    /// Reynolds number, Mach numbers, ground speed and track, flight path
    /// angle, total temperature and pressure, CAS and EAS.
    fn update_airspeeds(&mut self) {
        let (i, air) = (&self.input, &self.input.air);
        let s = &mut self.airspeed;
        s.re = self.vt * i.constants.wingchord / air.kinematic_viscosity;
        s.mach = self.vt / air.sound_speed;
        s.mach_uvw[E_U] = self.aero_uvw[E_U] / air.sound_speed;
        s.mach_u = s.mach_uvw[E_U];
        s.mach_uvw[E_V] = self.aero_uvw[E_V] / air.sound_speed;
        s.mach_uvw[E_W] = self.aero_uvw[E_W] / air.sound_speed;

        let v = i.vel;
        s.vground = (v[E_NORTH] * v[E_NORTH] + v[E_EAST] * v[E_EAST]).sqrt();
        self.psigt = libm::atan2(v[E_EAST], v[E_NORTH]);
        if self.psigt < 0.0 {
            self.psigt += 2.0 * std::f64::consts::PI;
        }
        self.gamma = libm::atan2(-v[E_DOWN], s.vground);

        s.tat = air.temperature * (1.0 + 0.2 * s.mach * s.mach);
        s.tatc = rankine_to_celsius(s.tat);
        s.pt = pitot_total_pressure(s.mach, air.pressure);

        if s.mach.abs() > 0.0 {
            // `VcalibratedFromMach` recomputes the total pressure just
            // stored in `pt`.
            s.vcas = airspeed::vcalibrated_from_total_pressure(
                s.pt,
                air.pressure,
                i.constants.std_day_sl_soundspeed,
            );
            s.veas = (2.0 * self.qbar / STD_DAY_SL_DENSITY).sqrt();
        } else {
            s.vcas = 0.0;
            s.veas = 0.0;
        }
    }

    /// CG load factors and the acceleration the pilot feels at the eye
    /// point.
    fn update_accelerations(&mut self) {
        let i = &self.input;
        let g = i.constants.standard_gravity;
        let l = &mut self.loads;
        l.ncg = i.body_accel / g;
        l.nz = -l.ncg[E_Z];
        l.ny = l.ncg[E_Y];
        l.nx = l.ncg[E_X];
        l.pilot_accel = i.body_accel + i.pqridot.cross(i.to_eye_pt);
        l.pilot_accel += i.pqri.cross(i.pqri.cross(i.to_eye_pt));

        l.nwcg = self.tb2w * l.ncg;
        l.nwcg[E_Z] = 1.0 - l.nwcg[E_Z];

        l.pilot_accel_n = l.pilot_accel / g;
    }

    /// The VRP position and the heights over span.
    fn update_reference_points(&mut self) {
        let i = &self.input;
        self.location_vrp = i.location.local_to_location(i.tb2l * i.vrp_body);
        self.location_vrp.cache_derived();
        let span = i.constants.wingspan;
        self.hoverbcg = i.distance_agl / span;
        let mac = i.tb2l * i.rp_body;
        self.hoverbmac = (i.distance_agl - mac[E_Z]) / span;
    }

    /// `GetNlf`: the load factor from the wind-axis lift, 0 without mass.
    #[must_use]
    pub fn nlf(&self) -> f64 {
        if self.input.mass != 0.0 {
            self.input.fw[E_LIFT] / (self.input.mass * SLUGTOLB)
        } else {
            0.0
        }
    }

    /// `GetMagBeta`, rad.
    #[must_use]
    pub fn mag_beta(&self) -> f64 {
        self.beta.abs()
    }
}

crate::snapshot::visit_fields! {
    Airspeeds { vcas, veas, vground, mach, mach_u, mach_uvw, qbar_uw, qbar_uv, re, pt, tat, tatc }
}
crate::snapshot::visit_fields! { Loads { pilot_accel, pilot_accel_n, ncg, nwcg, nx, ny, nz } }
crate::snapshot::visit_fields! { Start { neu_location } }
crate::snapshot::visit_fields! {
    Auxiliary {
        base, input, alpha, beta, adot, bdot, qbar, vt, tb2w, tw2b, aero_uvw, aero_pqr, euler_rates,
        psigt, gamma, hoverbcg, hoverbmac, location_vrp, airspeed, loads, start,
    }
}
