// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGPropagate.{h,cpp}.
//
//! Integration of the equations of motion: position, attitude, velocity
//! and rates in the ECI, ECEF, local and body frames.

mod bind;
mod integrate;
mod orbit;

pub use bind::bind;
pub use integrate::Integrator;

use super::base::ModelBase;
use super::inertial::Inertial;
use crate::math::{Axis, ColumnVector3, E_DOWN, E_Z, Location, Matrix33, Quaternion};
use crate::units::RADTODEG;

/// The integrated state (`VehicleState`).
#[derive(Debug, Clone)]
pub struct VehicleState {
    /// ECEF position.
    pub location: Location,
    /// Body velocity relative to the ECEF frame, ft/s.
    pub uvw: ColumnVector3,
    /// Body rates relative to the ECEF frame, rad/s.
    pub pqr: ColumnVector3,
    /// Body rates relative to the ECI frame, rad/s.
    pub pqri: ColumnVector3,
    /// Local-to-body attitude.
    pub q_attitude_local: Quaternion,
    /// ECI-to-body attitude.
    pub q_attitude_eci: Quaternion,
    /// Time derivative of `q_attitude_eci`.
    pub qtrndot: Quaternion,
    /// ECI position, ft.
    pub inertial_position: ColumnVector3,
    /// ECI velocity, ft/s.
    pub inertial_velocity: ColumnVector3,
    /// Derivative histories, newest first.
    pub dq_pqridot: [ColumnVector3; 5],
    /// See `dq_pqridot`.
    pub dq_uvwidot: [ColumnVector3; 5],
    /// See `dq_pqridot`.
    pub dq_inertial_velocity: [ColumnVector3; 5],
    /// See `dq_pqridot`.
    pub dq_qtrndot: [Quaternion; 5],
}

/// Inputs loaded by the executive (`FGPropagate::Inputs`).
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// Body angular acceleration relative to ECI.
    pub pqridot: ColumnVector3,
    /// ECI acceleration.
    pub uvwidot: ColumnVector3,
    /// Frame time step.
    pub delta_t: f64,
    /// Planet rotation.
    pub omega_planet: ColumnVector3,
    /// Planet semi-major axis.
    pub semi_major: f64,
    /// Planet semi-minor axis.
    pub semi_minor: f64,
    /// Gravitational parameter.
    pub gm: f64,
}

/// Orbital elements computed after each step.
#[derive(Debug, Clone, Copy, Default)]
pub struct Orbit {
    /// Specific angular momentum, ft^2/s.
    pub h: f64,
    /// Inclination, deg.
    pub inclination: f64,
    /// Right ascension of the ascending node, deg.
    pub right_ascension: f64,
    /// Eccentricity.
    pub eccentricity: f64,
    /// Argument of perigee, deg.
    pub perigee_argument: f64,
    /// True anomaly, deg.
    pub true_anomaly: f64,
    /// Apoapsis radius, ft.
    pub apoapsis_radius: f64,
    /// Periapsis radius, ft.
    pub periapsis_radius: f64,
    /// Orbital period, s.
    pub period: f64,
}

/// Frame transformations.
#[derive(Debug, Clone, Copy, Default)]
pub struct Frames {
    /// ECI to ECEF.
    pub ti2ec: Matrix33,
    /// ECEF to ECI.
    pub tec2i: Matrix33,
    /// Local to ECEF.
    pub tl2ec: Matrix33,
    /// ECEF to local.
    pub tec2l: Matrix33,
    /// ECI to local.
    pub ti2l: Matrix33,
    /// Local to ECI.
    pub tl2i: Matrix33,
    /// ECI to body.
    pub ti2b: Matrix33,
    /// Body to ECI.
    pub tb2i: Matrix33,
    /// Local to body.
    pub tl2b: Matrix33,
    /// Body to local.
    pub tb2l: Matrix33,
    /// ECEF to body.
    pub tec2b: Matrix33,
    /// Body to ECEF.
    pub tb2ec: Matrix33,
}

/// `FGPropagate`.
#[derive(Debug, Clone)]
pub struct Propagate {
    /// Scheduling.
    pub base: ModelBase,
    /// Inputs of the current frame.
    pub input: Inputs,
    /// The state.
    pub state: VehicleState,
    /// Frame transformations.
    pub frames: Frames,
    /// ECEF-to-body attitude.
    pub q_ec2b: Quaternion,
    /// Local (NED) velocity, ft/s.
    pub vel: ColumnVector3,
    /// Earth position angle, rad.
    pub epa: f64,
    /// Integrators, stored as the `int`s JSBSim ties.
    pub integrator_rotational_rate: i32,
    /// See `integrator_rotational_rate`.
    pub integrator_translational_rate: i32,
    /// See `integrator_rotational_rate`.
    pub integrator_rotational_position: i32,
    /// See `integrator_rotational_rate`.
    pub integrator_translational_position: i32,
    /// Orbital elements.
    pub orbit: Orbit,
    /// Terrain velocity under the vehicle, ECEF.
    pub local_terrain_velocity: ColumnVector3,
    /// Terrain angular velocity under the vehicle, ECEF.
    pub local_terrain_angular_velocity: ColumnVector3,
}

impl Default for Propagate {
    fn default() -> Self {
        let zero = ColumnVector3::zero();
        // FGQuaternion(0, 0, 0) is the Euler-angle constructor: identity.
        let identity = Quaternion::from_euler(0.0, 0.0, 0.0);
        Self {
            base: ModelBase::default(),
            input: Inputs::default(),
            state: VehicleState {
                location: Location::default(),
                uvw: zero,
                pqr: zero,
                pqri: zero,
                q_attitude_local: Quaternion::default(),
                q_attitude_eci: Quaternion::default(),
                qtrndot: Quaternion::default(),
                inertial_position: zero,
                inertial_velocity: zero,
                dq_pqridot: [zero; 5],
                dq_uvwidot: [zero; 5],
                dq_inertial_velocity: [zero; 5],
                dq_qtrndot: std::array::from_fn(|_| identity.clone()),
            },
            frames: Frames::default(),
            q_ec2b: Quaternion::default(),
            vel: zero,
            epa: 0.0,
            integrator_rotational_rate: Integrator::RectEuler as i32,
            integrator_translational_rate: Integrator::AdamsBashforth2 as i32,
            integrator_rotational_position: Integrator::RectEuler as i32,
            integrator_translational_position: Integrator::AdamsBashforth3 as i32,
            orbit: Orbit::default(),
            local_terrain_velocity: zero,
            local_terrain_angular_velocity: zero,
        }
    }
}

fn rotation_z(angle: f64) -> Matrix33 {
    let (c, s) = (libm::cos(angle), libm::sin(angle));
    Matrix33::new(c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0)
}

impl Propagate {
    /// `InitModel`. The derivative histories keep their values: JSBSim
    /// only resizes them, and they are already full.
    pub fn init_model(&mut self, inertial: &Inertial) {
        self.base.init();
        self.state
            .location
            .set_ellipse(self.input.semi_major, self.input.semi_minor);
        inertial.set_altitude_agl(&mut self.state.location, 4.0);
        self.integrator_rotational_rate = Integrator::RectEuler as i32;
        self.integrator_translational_rate = Integrator::AdamsBashforth2 as i32;
        self.integrator_rotational_position = Integrator::RectEuler as i32;
        self.integrator_translational_position = Integrator::AdamsBashforth3 as i32;
        self.epa = 0.0;
    }

    /// `SetInitialState`.
    #[allow(clippy::too_many_arguments)]
    pub fn set_initial_state(
        &mut self,
        inertial: &Inertial,
        position: Location,
        epa: f64,
        orientation: Quaternion,
        uvw: ColumnVector3,
        pqr: ColumnVector3,
    ) {
        self.state.location = position;
        self.epa = epa;
        self.frames.ti2ec = rotation_z(epa);
        self.frames.tec2i = self.frames.ti2ec.transposed();
        self.state.inertial_position = self.frames.tec2i * self.state.location.ecef();
        self.update_location_matrices();
        self.state.q_attitude_local = orientation;
        self.state.q_attitude_eci = &self.frames.ti2l.quaternion() * &self.state.q_attitude_local;
        self.update_body_matrices();
        self.state.uvw = uvw;
        self.vel = self.frames.tb2l * self.state.uvw;
        self.recompute_local_terrain_velocity(inertial);
        self.state.pqr = pqr;
        self.state.pqri = self.state.pqr + self.frames.ti2b * self.input.omega_planet;
        self.calculate_inertial_velocity();
        self.calculate_quatdot();
    }

    /// `InitializeDerivatives`: every history slot holds the current
    /// derivative.
    pub fn initialize_derivatives(&mut self) {
        self.state.dq_pqridot = [self.input.pqridot; 5];
        self.state.dq_uvwidot = [self.input.uvwidot; 5];
        self.state.dq_inertial_velocity = [self.state.inertial_velocity; 5];
        self.state.dq_qtrndot = std::array::from_fn(|_| self.state.qtrndot.clone());
    }

    /// `Run`.
    pub fn run(&mut self, holding: bool, integration_suspended: bool, inertial: &Inertial) {
        if self.base.skip() || holding {
            return;
        }
        let dt = self.input.delta_t * f64::from(self.base.rate);
        if !integration_suspended {
            self.integrate_all(dt);
        }
        self.epa += self.input.omega_planet[E_Z] * dt;
        self.frames.ti2ec = rotation_z(self.epa);
        self.frames.tec2i = self.frames.ti2ec.transposed();
        self.state
            .location
            .assign_ecef(self.frames.ti2ec * self.state.inertial_position);
        self.update_location_matrices();
        self.update_body_matrices();
        self.calculate_uvw();
        self.recompute_local_terrain_velocity(inertial);
        self.state.pqr = self.state.pqri - self.frames.ti2b * self.input.omega_planet;
        self.calculate_quatdot();
        self.state.q_attitude_local = self.frames.tl2b.quaternion();
        self.vel = self.frames.tb2l * self.state.uvw;
        self.compute_orbital_parameters();
    }

    /// `NudgeBodyLocation`: moves the vehicle by `delta` (body frame, ft).
    pub fn nudge_body_location(&mut self, delta: ColumnVector3) {
        self.state.inertial_position -= self.frames.tb2i * delta;
        self.state
            .location
            .sub_assign(&Location::from_ecef(self.frames.tb2ec * delta));
    }

    /// `SetHoldDown`.
    pub fn set_hold_down(&mut self, hd: bool) {
        if hd {
            self.state.uvw = ColumnVector3::zero();
            self.calculate_inertial_velocity();
            self.state.pqr = ColumnVector3::zero();
            self.state.pqri = self.frames.ti2b * self.input.omega_planet;
            self.calculate_quatdot();
            self.initialize_derivatives();
        }
    }

    fn calculate_quatdot(&mut self) {
        self.state.qtrndot = self.state.q_attitude_eci.q_dot(self.state.pqri);
    }

    fn calculate_inertial_velocity(&mut self) {
        self.state.inertial_velocity = self.frames.tb2i * self.state.uvw
            + self.input.omega_planet.cross(self.state.inertial_position);
    }

    fn calculate_uvw(&mut self) {
        self.state.uvw = self.frames.ti2b
            * (self.state.inertial_velocity
                - self.input.omega_planet.cross(self.state.inertial_position));
    }

    fn update_location_matrices(&mut self) {
        let f = &mut self.frames;
        f.tl2ec = self.state.location.tl2ec();
        f.tec2l = f.tl2ec.transposed();
        f.ti2l = f.tec2l * f.ti2ec;
        f.tl2i = f.ti2l.transposed();
    }

    fn update_body_matrices(&mut self) {
        let f = &mut self.frames;
        f.ti2b = self.state.q_attitude_eci.t();
        f.tb2i = f.ti2b.transposed();
        f.tl2b = f.ti2b * f.tl2i;
        f.tb2l = f.tl2b.transposed();
        f.tec2b = f.ti2b * f.tec2i;
        f.tb2ec = f.tec2b.transposed();
        self.q_ec2b = f.tec2b.quaternion();
    }

    fn recompute_local_terrain_velocity(&mut self, inertial: &Inertial) {
        let contact = inertial.contact(&self.state.location);
        self.local_terrain_velocity = contact.velocity;
        self.local_terrain_angular_velocity = contact.angular_velocity;
    }

    /// `UpdateVehicleState`: derived state after the location changed.
    pub fn update_vehicle_state(&mut self, inertial: &Inertial) {
        self.recompute_local_terrain_velocity(inertial);
        self.state.inertial_position = self.frames.tec2i * self.state.location.ecef();
        self.update_location_matrices();
        self.update_body_matrices();
        self.vel = self.frames.tb2l * self.state.uvw;
        self.state.q_attitude_local = self.frames.tl2b.quaternion();
    }

    /// `SetLocation`.
    pub fn set_location(&mut self, location: Location, inertial: &Inertial) {
        self.state.location = location;
        self.update_vehicle_state(inertial);
    }

    /// `GetAltitudeASL`.
    #[must_use]
    pub fn altitude_asl(&self) -> f64 {
        self.state.location.radius() - self.state.location.sea_level_radius()
    }

    /// `SetAltitudeASL`.
    pub fn set_altitude_asl(&mut self, alt: f64, inertial: &Inertial) {
        let slr = self.state.location.sea_level_radius();
        self.state.location.set_radius(slr + alt);
        self.update_vehicle_state(inertial);
    }

    /// `GetDistanceAGL`.
    #[must_use]
    pub fn distance_agl(&self, inertial: &Inertial) -> f64 {
        inertial.altitude_agl(&self.state.location)
    }

    /// `SetDistanceAGL`.
    pub fn set_distance_agl(&mut self, agl: f64, inertial: &Inertial) {
        inertial.set_altitude_agl(&mut self.state.location, agl);
        self.update_vehicle_state(inertial);
    }

    /// `GetTerrainElevation`.
    #[must_use]
    pub fn terrain_elevation(&self, inertial: &Inertial) -> f64 {
        inertial.contact(&self.state.location).point.geod_altitude()
    }

    /// `GetLocalTerrainRadius`.
    #[must_use]
    pub fn local_terrain_radius(&self, inertial: &Inertial) -> f64 {
        inertial.contact(&self.state.location).point.radius()
    }

    /// `GetRadius`: 1 at the planet centre.
    #[must_use]
    pub fn radius(&self) -> f64 {
        match self.state.location.radius() {
            0.0 => 1.0,
            r => r,
        }
    }

    /// `GetECEFVelocity`.
    #[must_use]
    pub fn ecef_velocity(&self) -> ColumnVector3 {
        self.frames.tb2ec * self.state.uvw
    }

    /// `GetEulerDeg(axis)`.
    #[must_use]
    pub fn euler_deg(&self, axis: Axis) -> f64 {
        self.state.q_attitude_local.euler()[axis] * RADTODEG
    }

    /// `Gethdot`.
    #[must_use]
    pub fn hdot(&self) -> f64 {
        -self.vel[E_DOWN]
    }

    /// `SetLongitude`.
    pub fn set_longitude(&mut self, lon: f64, inertial: &Inertial) {
        self.state.location.set_longitude(lon);
        self.update_vehicle_state(inertial);
    }

    /// `SetLatitude`.
    pub fn set_latitude(&mut self, lat: f64, inertial: &Inertial) {
        self.state.location.set_latitude(lat);
        self.update_vehicle_state(inertial);
    }

    /// `SetInertialOrientation`.
    pub fn set_inertial_orientation(&mut self, qi: Quaternion) {
        self.state.q_attitude_eci = qi;
        self.state.q_attitude_eci.normalize();
        self.update_body_matrices();
        self.state.q_attitude_local = self.frames.tl2b.quaternion();
        self.calculate_quatdot();
    }

    /// `SetInertialVelocity`.
    pub fn set_inertial_velocity(&mut self, vi: ColumnVector3) {
        self.state.inertial_velocity = vi;
        self.calculate_uvw();
        self.vel = self.frames.tb2l * self.state.uvw;
    }

    /// `SetInertialRates`.
    pub fn set_inertial_rates(&mut self, rates: ColumnVector3) {
        self.state.pqri = self.frames.ti2b * rates;
        self.state.pqr = self.state.pqri - self.frames.ti2b * self.input.omega_planet;
        self.calculate_quatdot();
    }

    /// `SetPQR(i, v)`.
    pub fn set_pqr(&mut self, i: Axis, v: f64) {
        self.state.pqr[i] = v;
        self.state.pqri = self.state.pqr + self.frames.ti2b * self.input.omega_planet;
    }

    /// `SetUVW(i, v)`.
    pub fn set_uvw(&mut self, i: Axis, v: f64) {
        self.state.uvw[i] = v;
        self.calculate_inertial_velocity();
    }
}
