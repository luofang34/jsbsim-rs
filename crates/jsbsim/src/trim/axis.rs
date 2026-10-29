// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGTrimAxis.{h,cpp}.
//
//! One trim axis (`FGTrimAxis`): a state to bring to its target (an
//! acceleration, the heading-minus-ground-track or the load factor) and
//! the control that moves it.

use crate::Error;
use crate::fdm::Fdm;
use crate::initial_condition::{Environment, InitialCondition};
use crate::math::{E_P, E_PHI, E_PSI, E_Q, E_R, E_THT, E_U, E_V, E_W};
use crate::units::{DEGTORAD, RADTODEG};

/// `DEFAULT_TOLERANCE`.
pub const DEFAULT_TOLERANCE: f64 = 0.001;

/// What an axis brings to its target (`State`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// `tAll`: no state; selects every axis for debugging.
    All,
    /// `tUdot`.
    Udot,
    /// `tVdot`.
    Vdot,
    /// `tWdot`.
    Wdot,
    /// `tQdot`.
    Qdot,
    /// `tPdot`.
    Pdot,
    /// `tRdot`.
    Rdot,
    /// `tHmgt`: heading minus ground track.
    Hmgt,
    /// `tNlf`: load factor.
    Nlf,
}

impl State {
    /// `StateNames`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Udot => "udot",
            Self::Vdot => "vdot",
            Self::Wdot => "wdot",
            Self::Qdot => "qdot",
            Self::Pdot => "pdot",
            Self::Rdot => "rdot",
            Self::Hmgt => "hmgt",
            Self::Nlf => "nlf",
        }
    }
}

/// What an axis moves (`Control`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// `tThrottle`: every engine's throttle, as a fraction of its range.
    Throttle,
    /// `tBeta`: the initial sideslip.
    Beta,
    /// `tAlpha`: the initial angle of attack.
    Alpha,
    /// `tElevator`.
    Elevator,
    /// `tAileron`.
    Aileron,
    /// `tRudder`.
    Rudder,
    /// `tAltAGL`: the initial height above ground.
    AltAgl,
    /// `tTheta`: the initial pitch.
    Theta,
    /// `tPhi`: the initial roll.
    Phi,
    /// `tGamma`: the initial flight path angle.
    Gamma,
    /// `tPitchTrim`.
    PitchTrim,
    /// `tRollTrim`, which moves the aileron command.
    RollTrim,
    /// `tYawTrim`, which moves the rudder command.
    YawTrim,
    /// `tHeading`: the initial heading.
    Heading,
}

impl Control {
    /// `ControlNames`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Throttle => "Throttle",
            Self::Beta => "Sideslip",
            Self::Alpha => "Angle of Attack",
            Self::Elevator => "Elevator",
            Self::Aileron => "Ailerons",
            Self::Rudder => "Rudder",
            Self::AltAgl => "Altitude AGL",
            Self::Theta => "Pitch Angle",
            Self::Phi => "Roll Angle",
            Self::Gamma => "Flight Path Angle",
            Self::PitchTrim => "Pitch Trim",
            Self::RollTrim => "Roll Trim",
            Self::YawTrim => "Yaw Trim",
            Self::Heading => "Heading",
        }
    }
}

/// `FGTrimAxis`.
#[derive(Debug, Clone, PartialEq)]
pub struct TrimAxis {
    state: State,
    control: Control,
    state_target: f64,
    state_value: f64,
    control_value: f64,
    control_min: f64,
    control_max: f64,
    tolerance: f64,
    solver_eps: f64,
    state_convert: f64,
    control_convert: f64,
    max_iterations: i32,
    its_to_stable_value: i32,
    total_stability_iterations: i32,
    total_iterations: i32,
}

impl TrimAxis {
    /// `FGTrimAxis(fdmex, ic, state, control)`: the tolerance of `state`
    /// and the range of `control`, some of which start from `ic`.
    #[must_use]
    pub fn new(fdm: &Fdm, ic: &InitialCondition, state: State, control: Control) -> Self {
        let mut axis = Self {
            state,
            control,
            state_target: 0.0,
            state_value: 0.0,
            control_value: 0.0,
            control_min: 0.0,
            control_max: 0.0,
            tolerance: DEFAULT_TOLERANCE,
            solver_eps: 0.0,
            state_convert: 1.0,
            control_convert: 1.0,
            max_iterations: 10,
            its_to_stable_value: 0,
            total_stability_iterations: 0,
            total_iterations: 0,
        };
        match state {
            State::Qdot | State::Pdot | State::Rdot => axis.tolerance = DEFAULT_TOLERANCE / 10.0,
            State::Hmgt => axis.tolerance = 0.01,
            State::Nlf => {
                axis.state_target = 1.0;
                axis.tolerance = 1E-5;
            }
            State::Udot | State::Vdot | State::Wdot | State::All => {}
        }
        axis.solver_eps = axis.tolerance;
        axis.set_ranges(fdm, ic);
        axis
    }

    /// The control range part of the constructor.
    fn set_ranges(&mut self, fdm: &Fdm, ic: &InitialCondition) {
        let m = &fdm.models;
        match self.control {
            Control::Throttle => {
                (self.control_min, self.control_max) = (0.0, 1.0);
                self.control_value = 0.5;
            }
            Control::Beta => {
                self.control_min = -30.0 * DEGTORAD;
                self.control_max = 30.0 * DEGTORAD;
                self.control_convert = RADTODEG;
            }
            Control::Alpha => {
                let a = &m.aerodynamics.state;
                (self.control_min, self.control_max) = (a.alpha_cl_min, a.alpha_cl_max);
                if self.control_max <= self.control_min {
                    self.control_max = 20.0 * DEGTORAD;
                    self.control_min = -5.0 * DEGTORAD;
                }
                self.control_value = (self.control_min + self.control_max) / 2.0;
                self.control_convert = RADTODEG;
                self.solver_eps = self.tolerance / 100.0;
            }
            Control::PitchTrim
            | Control::Elevator
            | Control::RollTrim
            | Control::Aileron
            | Control::YawTrim
            | Control::Rudder => {
                (self.control_min, self.control_max) = (-1.0, 1.0);
                self.state_convert = RADTODEG;
                self.solver_eps = self.tolerance / 100.0;
            }
            Control::AltAgl => {
                (self.control_min, self.control_max) = (0.0, 30.0);
                let env = Environment::new(&m.atmosphere, &m.inertial);
                self.control_value = ic.altitude_agl_ft(env);
                self.solver_eps = self.tolerance / 100.0;
            }
            Control::Theta => {
                let theta = ic.euler_rad(E_THT);
                self.control_min = theta - 5.0 * DEGTORAD;
                self.control_max = theta + 5.0 * DEGTORAD;
                self.state_convert = RADTODEG;
            }
            Control::Phi => {
                let phi = ic.euler_rad(E_PHI);
                self.control_min = phi - 30.0 * DEGTORAD;
                self.control_max = phi + 30.0 * DEGTORAD;
                self.state_convert = RADTODEG;
                self.control_convert = RADTODEG;
            }
            Control::Gamma => {
                self.solver_eps = self.tolerance / 100.0;
                self.control_min = -80.0 * DEGTORAD;
                self.control_max = 80.0 * DEGTORAD;
                self.control_convert = RADTODEG;
            }
            Control::Heading => {
                let psi = ic.euler_rad(E_PSI);
                self.control_min = psi - 30.0 * DEGTORAD;
                self.control_max = psi + 30.0 * DEGTORAD;
                self.state_convert = RADTODEG;
            }
        }
    }

    /// `getState`.
    fn read_state(&mut self, fdm: &Fdm) {
        let m = &fdm.models;
        let (uvwdot, pqrdot) = (m.accelerations.uvwdot, m.accelerations.pqrdot);
        self.state_value = match self.state {
            State::Udot => uvwdot[E_U],
            State::Vdot => uvwdot[E_V],
            State::Wdot => uvwdot[E_W],
            State::Qdot => pqrdot[E_Q],
            State::Pdot => pqrdot[E_P],
            State::Rdot => pqrdot[E_R],
            State::Hmgt => compute_hmgt(fdm),
            State::Nlf => m.auxiliary.nlf(),
            State::All => return,
        } - self.state_target;
    }

    /// `setControl`.
    fn apply_control(&self, fdm: &mut Fdm, ic: &mut InitialCondition) -> Result<(), Error> {
        let v = self.control_value;
        let m = &mut fdm.models;
        match self.control {
            Control::Throttle => return set_throttles_pct(v, fdm, ic),
            Control::Beta => ic.set_beta_rad(v),
            Control::Alpha => ic.set_alpha_rad(v),
            Control::PitchTrim => m.fcs.ptrim_cmd = v,
            Control::Elevator => m.fcs.de_cmd = v,
            Control::RollTrim | Control::Aileron => m.fcs.da_cmd = v,
            Control::YawTrim | Control::Rudder => m.fcs.dr_cmd = v,
            Control::AltAgl => {
                ic.set_altitude_agl_ft(Environment::new(&m.atmosphere, &m.inertial), v);
            }
            Control::Theta => ic.set_euler_angle_rad(E_THT, v),
            Control::Phi => ic.set_euler_angle_rad(E_PHI, v),
            Control::Gamma => ic.set_flight_path_angle_rad(v),
            Control::Heading => ic.set_euler_angle_rad(E_PSI, v),
        }
        Ok(())
    }

    /// `Run`: applies the control, then initializes from `ic` and runs a
    /// frame until the state settles (at most 100 times).
    pub fn run(&mut self, fdm: &mut Fdm, ic: &mut InitialCondition) -> Result<(), Error> {
        self.apply_control(fdm, ic)?;
        let mut i = 0i32;
        let mut stable = false;
        while !stable {
            i = i.wrapping_add(1);
            let last_state_value = self.state_value;
            fdm.initialize_from(ic.initial_state())?;
            fdm.run()?;
            self.read_state(fdm);
            if i > 1 && ((last_state_value - self.state_value).abs() < self.tolerance || i >= 100) {
                stable = true;
            }
        }
        self.its_to_stable_value = i;
        self.total_stability_iterations = self.total_stability_iterations.wrapping_add(i);
        self.total_iterations = self.total_iterations.wrapping_add(1);
        Ok(())
    }

    /// `GetState`: the state read now, less its target.
    pub fn state(&mut self, fdm: &Fdm) -> f64 {
        self.read_state(fdm);
        self.state_value
    }

    /// `InTolerance`.
    pub fn in_tolerance(&mut self, fdm: &Fdm) -> bool {
        self.read_state(fdm);
        self.state_value.abs() <= self.tolerance
    }

    /// `SetControl`.
    pub fn set_control(&mut self, value: f64) {
        self.control_value = value;
    }

    /// `GetControl`.
    #[must_use]
    pub fn control(&self) -> f64 {
        self.control_value
    }

    /// `GetStateType`.
    #[must_use]
    pub fn state_type(&self) -> State {
        self.state
    }

    /// `GetControlType`.
    #[must_use]
    pub fn control_type(&self) -> Control {
        self.control
    }

    /// `GetControlMin`.
    #[must_use]
    pub fn control_min(&self) -> f64 {
        self.control_min
    }

    /// `GetControlMax`.
    #[must_use]
    pub fn control_max(&self) -> f64 {
        self.control_max
    }

    /// `SetControlToMin`.
    pub fn set_control_to_min(&mut self) {
        self.control_value = self.control_min;
    }

    /// `SetControlToMax`.
    pub fn set_control_to_max(&mut self) {
        self.control_value = self.control_max;
    }

    /// `SetControlLimits`.
    pub fn set_control_limits(&mut self, min: f64, max: f64) {
        self.control_min = min;
        self.control_max = max;
    }

    /// `GetTolerance`.
    #[must_use]
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// `SetTolerance`.
    pub fn set_tolerance(&mut self, tolerance: f64) {
        self.tolerance = tolerance;
    }

    /// `GetSolverEps`.
    #[must_use]
    pub fn solver_eps(&self) -> f64 {
        self.solver_eps
    }

    /// `GetIterationLimit`, which the trim does not use.
    #[must_use]
    pub fn iteration_limit(&self) -> i32 {
        self.max_iterations
    }

    /// `GetStability`: frames the last run took to settle.
    #[must_use]
    pub fn stability(&self) -> i32 {
        self.its_to_stable_value
    }

    /// `GetRunCount`: frames of every run.
    #[must_use]
    pub fn run_count(&self) -> i32 {
        self.total_stability_iterations
    }

    /// `GetAvgStability`.
    #[must_use]
    pub fn avg_stability(&self) -> f64 {
        if self.total_iterations > 0 {
            return f64::from(self.total_stability_iterations) / f64::from(self.total_iterations);
        }
        0.0
    }

    /// `SetStateTarget`.
    pub fn set_state_target(&mut self, target: f64) {
        self.state_target = target;
    }

    /// `GetStateTarget`.
    #[must_use]
    pub fn state_target(&self) -> f64 {
        self.state_target
    }

    /// The factors `AxisReport` shows the control and state in.
    #[must_use]
    pub fn converts(&self) -> (f64, f64) {
        (self.control_convert, self.state_convert)
    }
}

/// `computeHmgt`: heading minus ground track, wrapped into [-pi, pi].
fn compute_hmgt(fdm: &Fdm) -> f64 {
    use std::f64::consts::PI;
    let m = &fdm.models;
    let diff = m.propagate.state.q_attitude_local.euler()[E_PSI] - m.auxiliary.psigt;
    if diff < -PI {
        diff + 2.0 * PI
    } else if diff > PI {
        diff - 2.0 * PI
    } else {
        diff
    }
}

/// `setThrottlesPct`: each engine's throttle command and position at
/// `pct` of its range, each followed by a frame and the engines' steady
/// state.
fn set_throttles_pct(pct: f64, fdm: &mut Fdm, ic: &InitialCondition) -> Result<(), Error> {
    for i in 0..fdm.models.propulsion.engines.len() {
        let Some(e) = fdm.models.propulsion.engines.get(i) else {
            continue;
        };
        let (t_min, t_max) = (e.base.min_throttle, e.base.max_throttle);
        let n = i32::try_from(i).unwrap_or(i32::MAX);
        let m = &mut fdm.models;
        m.fcs
            .throttles
            .set_throttle_cmd(n, t_min + pct * (t_max - t_min));
        if let Some(pos) = m.propulsion.input.throttle_pos.get_mut(i) {
            *pos = t_min + pct * (t_max - t_min);
        }
        fdm.initialize_from(ic.initial_state())?;
        fdm.run()?;
        fdm.steady_state()?;
    }
    Ok(())
}
