// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGInitialCondition.{h,cpp}.
//
//! Initial conditions (`FGInitialCondition`): a kinematically consistent set
//! of position, orientation, velocities and wind built from whichever
//! quantities were set last.
//!
//! The state is kept as JSBSim keeps it: the ground velocity in the local
//! (NED) frame, the true airspeed magnitude and the wind-to-body matrix, from
//! which the wind is the difference between the airspeed and the ground
//! velocity. Every setter keeps the quantities JSBSim keeps; see the
//! submodules for which ones.
//!
//! Setters and getters take the atmosphere (for calibrated, equivalent and
//! Mach speeds) and the planet (for its ellipsoid and terrain) explicitly,
//! because the initial conditions live next to those models in
//! [`Models`](crate::fdm::Models).

mod attitude;
mod bind;
mod load;
mod load_v2;
mod position;
mod speed;
mod wind;

#[cfg(test)]
mod tests;

pub use bind::bind;
pub use load::LoadOutcome;

use crate::fdm::InitialState;
use crate::math::{Axis, ColumnVector3, E_U, E_V, E_W, Location, Matrix33, Quaternion};
use crate::models::atmosphere::Atmosphere;
use crate::models::inertial::Inertial;
use crate::units::{FPSTOKTS, RADTODEG};

/// Which speed was set last (`speedset`); altitude changes keep it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpeedSet {
    /// True airspeed (`setvt`).
    #[default]
    Vt,
    /// Calibrated airspeed (`setvc`).
    Vc,
    /// Equivalent airspeed (`setve`).
    Ve,
    /// Mach number (`setmach`).
    Mach,
    /// Body velocity components (`setuvw`).
    Uvw,
    /// Local (NED) velocity components (`setned`).
    Ned,
    /// Ground speed (`setvg`).
    Vg,
}

/// Which altitude was set last (`altitudeset`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AltitudeSet {
    /// Above sea level (`setasl`).
    #[default]
    Asl,
    /// Above ground level (`setagl`).
    Agl,
}

/// Which latitude was set last (`latitudeset`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LatitudeSet {
    /// Geocentric (`setgeoc`).
    #[default]
    Geocentric,
    /// Geodetic (`setgeod`).
    Geodetic,
}

/// The models the initial conditions read: the atmosphere for airspeed
/// conversions and the planet for its shape and terrain.
#[derive(Debug, Clone, Copy)]
pub struct Environment<'a> {
    /// `FGAtmosphere`.
    pub atmosphere: &'a Atmosphere,
    /// `FGInertial`, with the ground callback.
    pub inertial: &'a Inertial,
}

/// `FGInitialCondition`.
#[derive(Debug, Clone)]
pub struct InitialCondition {
    /// Ground velocity, local NED frame, ft/s.
    uvw_ned: ColumnVector3,
    /// Body rates, rad/s.
    pqr_body: ColumnVector3,
    position: Location,
    /// Local-to-body orientation.
    orientation: Quaternion,
    /// True airspeed, ft/s.
    vt: f64,
    target_nlf: f64,
    tw2b: Matrix33,
    tb2w: Matrix33,
    alpha: f64,
    beta: f64,
    /// Earth position angle, rad.
    epa: f64,
    last_speed_set: SpeedSet,
    last_altitude_set: AltitudeSet,
    last_latitude_set: LatitudeSet,
}

impl<'a> Environment<'a> {
    /// The atmosphere and planet the initial conditions read.
    #[must_use]
    pub fn new(atmosphere: &'a Atmosphere, inertial: &'a Inertial) -> Self {
        Self {
            atmosphere,
            inertial,
        }
    }
}

const IDENTITY: Matrix33 = Matrix33::new(1., 0., 0., 0., 1., 0., 0., 0., 1.);

impl Default for InitialCondition {
    /// The state before `InitializeIC` gives the position an ellipse.
    fn default() -> Self {
        Self {
            uvw_ned: ColumnVector3::zero(),
            pqr_body: ColumnVector3::zero(),
            position: Location::default(),
            orientation: Quaternion::default(),
            vt: 0.0,
            target_nlf: 1.0,
            tw2b: IDENTITY,
            tb2w: IDENTITY,
            alpha: 0.0,
            beta: 0.0,
            epa: 0.0,
            last_speed_set: SpeedSet::Vt,
            last_altitude_set: AltitudeSet::Asl,
            last_latitude_set: LatitudeSet::Geocentric,
        }
    }
}

/// The wind-to-body matrix for the given cosines and sines of alpha and
/// beta.
fn wind_to_body(calpha: f64, salpha: f64, cbeta: f64, sbeta: f64) -> Matrix33 {
    Matrix33::new(
        calpha * cbeta,
        -calpha * sbeta,
        -salpha,
        sbeta,
        cbeta,
        0.0,
        salpha * cbeta,
        -salpha * sbeta,
        calpha,
    )
}

impl InitialCondition {
    /// `FGInitialCondition(fdmex)`: the initial state on the planet of
    /// `inertial`.
    #[must_use]
    pub fn new(inertial: &Inertial) -> Self {
        let mut ic = Self::default();
        ic.initialize_ic(inertial);
        ic
    }

    /// `InitializeIC`: at rest on the equator at sea level, level and
    /// heading north, with the ellipsoid of `inertial`.
    pub fn initialize_ic(&mut self, inertial: &Inertial) {
        *self = Self::default();
        self.position.set_ellipse(inertial.a, inertial.b);
        self.position.set_position_geodetic(0.0, 0.0, 0.0);
        self.orientation = Quaternion::from_euler(0.0, 0.0, 0.0);
    }

    /// `ResetIC`: body velocities and rates, aerodynamic and Euler angles,
    /// geocentric position with altitude above ground, then the flight
    /// path angle.
    #[allow(clippy::too_many_arguments)]
    pub fn reset_ic(
        &mut self,
        env: Environment<'_>,
        uvw: ColumnVector3,
        pqr: ColumnVector3,
        alpha0: f64,
        beta0: f64,
        euler: ColumnVector3,
        latitude_rad: f64,
        longitude_rad: f64,
        altitude_agl_ft: f64,
        gamma0: f64,
    ) {
        let (calpha, cbeta) = (libm::cos(alpha0), libm::cos(beta0));
        let (salpha, sbeta) = (libm::sin(alpha0), libm::sin(beta0));
        self.initialize_ic(env.inertial);
        self.pqr_body = pqr;
        self.alpha = alpha0;
        self.beta = beta0;
        self.position.set_longitude(longitude_rad);
        self.position.set_latitude(latitude_rad);
        env.inertial
            .set_altitude_agl(&mut self.position, altitude_agl_ft);
        self.last_latitude_set = LatitudeSet::Geocentric;
        self.last_altitude_set = AltitudeSet::Agl;
        self.orientation = Quaternion::from_euler_vector(euler);
        let tb2l = self.orientation.t_inv();
        self.uvw_ned = tb2l * uvw;
        self.vt = self.uvw_ned.magnitude();
        self.last_speed_set = SpeedSet::Uvw;
        self.tw2b = wind_to_body(calpha, salpha, cbeta, sbeta);
        self.tb2w = self.tw2b.transposed();
        self.set_flight_path_angle_rad(gamma0);
    }

    /// The true airspeed vector in the local frame.
    fn vt_ned(&self) -> ColumnVector3 {
        self.orientation.t_inv() * self.tw2b * ColumnVector3::new(self.vt, 0., 0.)
    }

    /// `calcAeroAngles`: alpha, beta and the wind-to-body matrix from the
    /// true airspeed vector in the local frame.
    fn calc_aero_angles(&mut self, vt_ned: ColumnVector3) {
        let vt_body = self.orientation.t() * vt_ned;
        let (ua, va, wa) = (vt_body[E_U], vt_body[E_V], vt_body[E_W]);
        let uwa = (ua * ua + wa * wa).sqrt();
        self.alpha = 0.0;
        self.beta = 0.0;
        let (mut calpha, mut cbeta) = (1.0, 1.0);
        let (mut salpha, mut sbeta) = (0.0, 0.0);
        if wa != 0.0 {
            self.alpha = libm::atan2(wa, ua);
        }
        if va != 0.0 {
            self.beta = libm::atan2(va, uwa);
        }
        if uwa != 0.0 {
            calpha = ua / uwa;
            salpha = wa / uwa;
        }
        if self.vt != 0.0 {
            cbeta = uwa / self.vt;
            sbeta = va / self.vt;
        }
        self.tw2b = wind_to_body(calpha, salpha, cbeta, sbeta);
        self.tb2w = self.tw2b.transposed();
    }

    /// What `FGFDMExec::Initialize` hands the models.
    #[must_use]
    pub fn initial_state(&self) -> InitialState {
        InitialState {
            position: self.position.clone(),
            epa: self.epa,
            orientation: self.orientation.clone(),
            uvw: self.uvw_fps(),
            pqr: self.pqr_body,
            wind_ned: self.wind_ned_fps(),
        }
    }

    /// `GetPosition`.
    #[must_use]
    pub fn position(&self) -> &Location {
        &self.position
    }

    /// `GetOrientation`.
    #[must_use]
    pub fn orientation(&self) -> &Quaternion {
        &self.orientation
    }

    /// `GetEarthPositionAngleIC`, rad.
    #[must_use]
    pub fn earth_position_angle(&self) -> f64 {
        self.epa
    }

    /// `GetSpeedSet`.
    #[must_use]
    pub fn speed_set(&self) -> SpeedSet {
        self.last_speed_set
    }

    /// `GetTargetNlfIC`.
    #[must_use]
    pub fn target_nlf(&self) -> f64 {
        self.target_nlf
    }

    /// `SetTargetNlfIC`.
    pub fn set_target_nlf(&mut self, nlf: f64) {
        self.target_nlf = nlf;
    }

    /// `GetVtrueFpsIC`.
    #[must_use]
    pub fn vtrue_fps(&self) -> f64 {
        self.vt
    }

    /// `GetVtrueKtsIC`.
    #[must_use]
    pub fn vtrue_kts(&self) -> f64 {
        self.vt * FPSTOKTS
    }

    /// `GetVgroundFpsIC`: horizontal ground speed.
    #[must_use]
    pub fn vground_fps(&self) -> f64 {
        self.uvw_ned.magnitude2(E_U, E_V)
    }

    /// `GetVgroundKtsIC`.
    #[must_use]
    pub fn vground_kts(&self) -> f64 {
        self.vground_fps() * FPSTOKTS
    }

    /// `GetUVWFpsIC`: ground velocity in the body frame.
    #[must_use]
    pub fn uvw_fps(&self) -> ColumnVector3 {
        self.orientation.t() * self.uvw_ned
    }

    /// `GetUBodyFpsIC`, `GetVBodyFpsIC`, `GetWBodyFpsIC` by axis.
    #[must_use]
    pub fn body_vel_fps(&self, idx: Axis) -> f64 {
        self.uvw_fps()[idx]
    }

    /// `GetVNorthFpsIC`, `GetVEastFpsIC`, `GetVDownFpsIC` by axis.
    #[must_use]
    pub fn ned_vel_fps(&self, idx: Axis) -> f64 {
        self.uvw_ned[idx]
    }

    /// `GetPQRRadpsIC`.
    #[must_use]
    pub fn pqr_radps(&self) -> ColumnVector3 {
        self.pqr_body
    }

    /// `SetPRadpsIC`, `SetQRadpsIC`, `SetRRadpsIC` by axis (`E_P`, `E_Q`,
    /// `E_R`).
    pub fn set_pqr_radps(&mut self, idx: Axis, rate: f64) {
        self.pqr_body[idx] = rate;
    }

    /// `GetAlphaRadIC`.
    #[must_use]
    pub fn alpha_rad(&self) -> f64 {
        self.alpha
    }

    /// `GetAlphaDegIC`.
    #[must_use]
    pub fn alpha_deg(&self) -> f64 {
        self.alpha * RADTODEG
    }

    /// `GetBetaRadIC`.
    #[must_use]
    pub fn beta_rad(&self) -> f64 {
        self.beta
    }

    /// `GetBetaDegIC`.
    #[must_use]
    pub fn beta_deg(&self) -> f64 {
        self.beta * RADTODEG
    }

    /// `GetPhiRadIC`, `GetThetaRadIC`, `GetPsiRadIC` by angle (`E_PHI`,
    /// `E_THT`, `E_PSI`).
    #[must_use]
    pub fn euler_rad(&self, idx: Axis) -> f64 {
        self.orientation.euler()[idx]
    }

    /// `GetPhiDegIC`, `GetThetaDegIC`, `GetPsiDegIC` by angle.
    #[must_use]
    pub fn euler_deg(&self, idx: Axis) -> f64 {
        self.orientation.euler_deg()[idx]
    }

    /// `GetClimbRateFpsIC`: the upward component of the true airspeed.
    #[must_use]
    pub fn climb_rate_fps(&self) -> f64 {
        -self.vt_ned()[E_W]
    }

    /// `GetClimbRateFpmIC`.
    #[must_use]
    pub fn climb_rate_fpm(&self) -> f64 {
        self.climb_rate_fps() * 60.0
    }

    /// `GetFlightPathAngleRadIC`; 0 at zero airspeed.
    #[must_use]
    pub fn flight_path_angle_rad(&self) -> f64 {
        if self.vt == 0.0 {
            0.0
        } else {
            libm::asin(self.climb_rate_fps() / self.vt)
        }
    }

    /// `GetFlightPathAngleDegIC`.
    #[must_use]
    pub fn flight_path_angle_deg(&self) -> f64 {
        self.flight_path_angle_rad() * RADTODEG
    }
}
