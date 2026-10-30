// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGTrim.{h,cpp}.
//
//! The trim (`FGTrim`): finds the controls and initial conditions that
//! zero chosen accelerations, one axis at a time.
//!
//! The trim works on its own copy of the executive's initial conditions:
//! each axis run initializes the models from that copy and runs a frame
//! with integration suspended. The executive's initial conditions are
//! left as they were, so a reset returns to the untrimmed state; the
//! trimmed state lives in the models and the flight controls.
//!
//! Each cycle visits the axes in mode order: an axis without a bracket
//! first checks its control range for a sign change, one with a bracket
//! searches around its current control, and either is then solved by
//! regula falsi. The trim succeeds once every axis is within
//! its tolerance and fails after 60 cycles, or when all but one axis are
//! within tolerance and the last cannot change sign over its range.

mod axis;
mod ground;
mod maneuver;
mod report;
mod solve;

#[cfg(test)]
mod tests;

pub(crate) use axis::TrimAxis;
pub use axis::{Control, State};
pub use report::{AxisReport, TrimReport};
pub(crate) use solve::Solver;

use crate::Error;
use crate::fdm::Fdm;
use crate::initial_condition::InitialCondition;
use crate::math::{E_P, E_PHI, E_Q, E_R, E_THT};
use crate::units::DEGTORAD;

/// `TrimMode`, with JSBSim's values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimMode {
    /// `tLongitudinal`: wdot with alpha, udot with throttle, qdot with
    /// pitch trim.
    Longitudinal = 0,
    /// `tFull`: longitudinal, then vdot with phi, pdot with aileron, rdot
    /// with rudder.
    Full = 1,
    /// `tGround`: wdot with altitude, qdot with theta, pdot with phi.
    Ground = 2,
    /// `tPullup`: the load factor with alpha at the pitch rate it needs.
    Pullup = 3,
    /// `tCustom`: the axes added with `Trim::add_state`.
    Custom = 4,
    /// `tTurn`: a coordinated turn at the initial bank angle.
    Turn = 5,
    /// `tNone`.
    None = 6,
}

impl TrimMode {
    /// The mode of JSBSim value `mode`, if any.
    #[must_use]
    pub fn from_i32(mode: i32) -> Option<Self> {
        Some(match mode {
            0 => Self::Longitudinal,
            1 => Self::Full,
            2 => Self::Ground,
            3 => Self::Pullup,
            4 => Self::Custom,
            5 => Self::Turn,
            6 => Self::None,
            _ => return None,
        })
    }
}

/// The flight control commands a failed trim puts back.
struct Commands {
    throttle: Vec<f64>,
    elevator: f64,
    aileron: f64,
    rudder: f64,
    pitch_trim: f64,
}

#[doc(hidden)]
/// `FGTrim`.
#[derive(Debug, Clone)]
pub struct Trim {
    axes: Vec<TrimAxis>,
    mode: TrimMode,
    fgic: InitialCondition,
    solver: Solver,
    debug_level: i32,
    sub_iterations: Vec<f64>,
    successful: Vec<f64>,
    solution: Vec<bool>,
    max_iterations: u32,
    total_its: u32,
    gamma_fallback: bool,
    target_nlf: f64,
    debug_axis: State,
    psidot: f64,
    success: bool,
}

impl Trim {
    /// `FGTrim(fdmex, mode)`: a copy of the executive's initial
    /// conditions and the axes of `mode`.
    #[must_use]
    pub fn new(fdm: &Fdm, mode: TrimMode) -> Self {
        let fgic = fdm.models.ic.clone();
        let mut trim = Self {
            axes: Vec::new(),
            mode,
            target_nlf: fgic.target_nlf(),
            fgic,
            solver: Solver::default(),
            debug_level: 0,
            sub_iterations: Vec::new(),
            successful: Vec::new(),
            solution: Vec::new(),
            max_iterations: 60,
            total_its: 0,
            gamma_fallback: false,
            debug_axis: State::All,
            psidot: 0.0,
            success: false,
        };
        trim.set_mode(fdm, mode);
        trim
    }

    /// `SetMode`: the axes of `mode` replace the current ones.
    pub fn set_mode(&mut self, fdm: &Fdm, mode: TrimMode) {
        self.clear_states();
        self.mode = mode;
        use {Control as C, State as S};
        let pairs: &[(S, C)] = match mode {
            TrimMode::Full => &[
                (S::Wdot, C::Alpha),
                (S::Udot, C::Throttle),
                (S::Qdot, C::PitchTrim),
                (S::Vdot, C::Phi),
                (S::Pdot, C::Aileron),
                (S::Rdot, C::Rudder),
            ],
            TrimMode::Longitudinal => &[
                (S::Wdot, C::Alpha),
                (S::Udot, C::Throttle),
                (S::Qdot, C::PitchTrim),
            ],
            TrimMode::Ground => &[(S::Wdot, C::AltAgl), (S::Qdot, C::Theta), (S::Pdot, C::Phi)],
            TrimMode::Pullup => &[
                (S::Nlf, C::Alpha),
                (S::Udot, C::Throttle),
                (S::Qdot, C::PitchTrim),
                (S::Hmgt, C::Beta),
                (S::Vdot, C::Phi),
                (S::Pdot, C::Aileron),
                (S::Rdot, C::Rudder),
            ],
            TrimMode::Turn => &[
                (S::Wdot, C::Alpha),
                (S::Udot, C::Throttle),
                (S::Qdot, C::PitchTrim),
                (S::Vdot, C::Beta),
                (S::Pdot, C::Aileron),
                (S::Rdot, C::Rudder),
            ],
            TrimMode::Custom | TrimMode::None => &[],
        };
        for &(state, control) in pairs {
            self.axes
                .push(TrimAxis::new(fdm, &self.fgic, state, control));
        }
        self.resize();
    }

    fn resize(&mut self) {
        let n = self.axes.len();
        self.sub_iterations.resize(n, 0.0);
        self.successful.resize(n, 0.0);
        self.solution.resize(n, false);
    }

    /// `ClearStates`.
    pub fn clear_states(&mut self) {
        self.mode = TrimMode::Custom;
        self.axes.clear();
    }

    /// `AddState`: false when `state` already has an axis.
    pub fn add_state(&mut self, fdm: &Fdm, state: State, control: Control) -> bool {
        self.mode = TrimMode::Custom;
        if self.axes.iter().any(|a| a.state_type() == state) {
            return false;
        }
        self.axes
            .push(TrimAxis::new(fdm, &self.fgic, state, control));
        self.resize();
        true
    }

    /// `RemoveState`.
    pub fn remove_state(&mut self, state: State) -> bool {
        self.mode = TrimMode::Custom;
        let before = self.axes.len();
        self.axes.retain(|a| a.state_type() != state);
        let removed = self.axes.len() != before;
        if removed {
            self.resize();
        }
        removed
    }

    /// `EditState`: the axis of `state` gets `control`, afresh.
    pub fn edit_state(&mut self, fdm: &Fdm, state: State, control: Control) -> bool {
        self.mode = TrimMode::Custom;
        let fresh = TrimAxis::new(fdm, &self.fgic, state, control);
        match self.axes.iter_mut().find(|a| a.state_type() == state) {
            Some(axis) => {
                *axis = fresh;
                true
            }
            None => false,
        }
    }

    /// `SetGammaFallback`: when udot cannot be trimmed with the throttle,
    /// trim it with the flight path angle.
    pub fn set_gamma_fallback(&mut self, on: bool) {
        self.gamma_fallback = on;
    }

    /// `SetMaxCycles`.
    pub fn set_max_cycles(&mut self, n: u32) {
        self.max_iterations = n;
    }

    /// `SetMaxCyclesPerAxis`.
    pub fn set_max_cycles_per_axis(&mut self, n: u32) {
        self.solver.max_sub_iterations = n;
    }

    /// `SetDebug`: 1 reports each cycle, 2 each solver step too.
    pub fn set_debug(&mut self, level: i32) {
        self.debug_level = level;
    }

    /// `DebugState`: limits the debug output to the axis of `state`.
    pub fn debug_state(&mut self, state: State) {
        self.debug_axis = state;
    }

    /// `SetTargetNlf`.
    pub fn set_target_nlf(&mut self, nlf: f64) {
        self.target_nlf = nlf;
    }

    /// `GetTargetNlf`.
    #[must_use]
    pub fn target_nlf(&self) -> f64 {
        self.target_nlf
    }

    /// The axes, in trim order.
    #[must_use]
    pub fn axes(&self) -> &[TrimAxis] {
        &self.axes
    }

    /// `DoTrim`: true when every axis ends within its tolerance. On
    /// failure the flight controls and the initial conditions are put
    /// back and the models initialized from them (and set on the ground
    /// again when on it). Integration is suspended throughout.
    pub fn do_trim(&mut self, fdm: &mut Fdm) -> Result<bool, Error> {
        let fcs = &fdm.models.fcs;
        let saved = Commands {
            throttle: fcs.throttles.throttle_cmd.clone(),
            elevator: fcs.de_cmd,
            aileron: fcs.da_cmd,
            rudder: fcs.dr_cmd,
            pitch_trim: fcs.ptrim_cmd,
        };
        set_gear_reports(fdm, false);
        fdm.models.exec.trim_status = true;
        let e = &mut fdm.models.exec;
        e.saved_dt = e.dt;
        e.dt = 0.0;
        self.fgic.set_pqr_radps(E_P, 0.0);
        self.fgic.set_pqr_radps(E_Q, 0.0);
        self.fgic.set_pqr_radps(E_R, 0.0);
        let failed = self.trim_loop(fdm)?;
        if failed {
            self.restore(fdm, &saved)?;
        }
        if fdm.models.exec.debug_level > 0 {
            tracing::debug!("\n  Trim {}", if failed { "failed" } else { "successful" });
        }
        fdm.models.propagate.initialize_derivatives();
        let e = &mut fdm.models.exec;
        e.dt = e.saved_dt;
        e.trim_status = false;
        set_gear_reports(fdm, true);
        self.success = !failed;
        Ok(!failed)
    }

    /// `DoTrim` from the ground preparation to the end of the cycles:
    /// whether the trim failed.
    fn trim_loop(&mut self, fdm: &mut Fdm) -> Result<bool, Error> {
        if self.mode == TrimMode::Ground {
            fdm.initialize_from(self.fgic.initial_state())?;
            fdm.run()?;
            self.trim_on_ground(fdm)?;
            let m = &fdm.models;
            let env = crate::initial_condition::Environment::new(&m.atmosphere, &m.inertial);
            let agl = self.fgic.altitude_agl_ft(env);
            let theta = self.fgic.euler_rad(E_THT);
            let phi = self.fgic.euler_rad(E_PHI);
            if let [a0, a1, a2, ..] = self.axes.as_mut_slice() {
                a0.set_control_limits(0.0, agl);
                a1.set_control_limits(theta - 5.0 * DEGTORAD, theta + 5.0 * DEGTORAD);
                a2.set_control_limits(phi - 30.0 * DEGTORAD, phi + 30.0 * DEGTORAD);
            }
        }
        for i in 0..self.axes.len() {
            let Some(axis) = self.axes.get_mut(i) else {
                continue;
            };
            self.solver.xlo = axis.control_min();
            self.solver.xhi = axis.control_max();
            axis.set_control((self.solver.xlo + self.solver.xhi) / 2.0);
            axis.run(fdm, &mut self.fgic)?;
            set(&mut self.sub_iterations, i, 0.0);
            set(&mut self.successful, i, 0.0);
            set(&mut self.solution, i, false);
        }
        match self.mode {
            TrimMode::Pullup => {
                tracing::info!("Setting pitch rate and nlf...");
                self.setup_pullup(fdm);
                tracing::info!("pitch rate done ...");
                if let Some(a) = self.axes.first_mut() {
                    a.set_state_target(self.target_nlf);
                }
                tracing::info!("nlf done");
            }
            TrimMode::Turn => self.setup_turn(fdm),
            _ => {}
        }
        let mut n = 0u32;
        loop {
            let axis_count = self.cycle(fdm)?;
            let mut failed = false;
            if axis_count == self.axes.len().wrapping_sub(1) && self.axes.len() > 1 {
                failed = self.last_axis(fdm, n)?;
            }
            n = n.wrapping_add(1);
            if n > self.max_iterations {
                failed = true;
            }
            if axis_count >= self.axes.len() || failed {
                self.total_its = n;
                return Ok(failed || axis_count < self.axes.len());
            }
        }
    }

    /// One cycle over the axes: the number in tolerance after it.
    fn cycle(&mut self, fdm: &mut Fdm) -> Result<usize, Error> {
        for i in 0..self.axes.len() {
            let Some(state) = self.axes.get(i).map(TrimAxis::state_type) else {
                continue;
            };
            self.solver.debug = if self.debug_axis == State::All || state == self.debug_axis {
                self.debug_level
            } else {
                0
            };
            self.update_rates_of(fdm);
            let (Some(axis), Some(solved)) = (self.axes.get_mut(i), self.solution.get_mut(i))
            else {
                continue;
            };
            let s = &mut self.solver;
            let ic = &mut self.fgic;
            s.nsub = 0;
            if !*solved {
                if s.check_limits(axis, fdm, ic)? {
                    *solved = true;
                    s.solve(axis, fdm, ic)?;
                }
            } else if s.find_interval(axis, fdm, ic)? {
                s.solve(axis, fdm, ic)?;
            } else {
                *solved = false;
            }
            if let Some(sub) = self.sub_iterations.get_mut(i) {
                *sub += f64::from(s.nsub);
            }
        }
        let mut axis_count = 0usize;
        for (axis, ok) in self.axes.iter_mut().zip(&mut self.successful) {
            if self.debug_level > 0 && self.solver.debug > 0 {
                report::log_axis(axis, fdm);
            }
            if axis.in_tolerance(fdm) {
                axis_count = axis_count.wrapping_add(1);
                *ok += 1.0;
            }
        }
        Ok(axis_count)
    }

    /// The all-but-one check: the axis out of tolerance that cannot change
    /// sign over its range fails the trim, or with the gamma fallback
    /// becomes udot with the flight path angle. Whether the trim failed.
    fn last_axis(&mut self, fdm: &mut Fdm, n: u32) -> Result<bool, Error> {
        let mut failed = false;
        for i in 0..self.axes.len() {
            let Some(axis) = self.axes.get_mut(i) else {
                continue;
            };
            if axis.in_tolerance(fdm) || self.solver.check_limits(axis, fdm, &mut self.fgic)? {
                continue;
            }
            if self.gamma_fallback
                && axis.state_type() == State::Udot
                && axis.control_type() == Control::Throttle
            {
                tracing::warn!("  Can't trim udot with throttle, trying flight path angle. ({n})");
                if axis.state(fdm) > 0.0 {
                    axis.set_control_to_min();
                } else {
                    axis.set_control_to_max();
                }
                axis.run(fdm, &mut self.fgic)?;
                *axis = TrimAxis::new(fdm, &self.fgic, State::Udot, Control::Gamma);
            } else {
                tracing::error!(
                    "  Sorry, {} doesn't appear to be trimmable",
                    axis.state_type().name()
                );
                failed = true;
            }
        }
        Ok(failed)
    }

    /// The failure branch of `DoTrim`.
    fn restore(&mut self, fdm: &mut Fdm, saved: &Commands) -> Result<(), Error> {
        self.fgic = fdm.models.ic.clone();
        let fcs = &mut fdm.models.fcs;
        fcs.de_cmd = saved.elevator;
        fcs.da_cmd = saved.aileron;
        fcs.dr_cmd = saved.rudder;
        fcs.ptrim_cmd = saved.pitch_trim;
        for (i, &t) in saved.throttle.iter().enumerate() {
            let n = i32::try_from(i).unwrap_or(i32::MAX);
            fcs.throttles.set_throttle_cmd(n, t);
        }
        fdm.initialize_from(self.fgic.initial_state())?;
        fdm.run()?;
        if fdm.models.ground_reactions.wow() {
            self.trim_on_ground(fdm)?;
        }
        Ok(())
    }
}

fn set<T>(v: &mut [T], i: usize, value: T) {
    if let Some(x) = v.get_mut(i) {
        *x = value;
    }
}

/// `SetReport` on every gear unit.
fn set_gear_reports(fdm: &mut Fdm, on: bool) {
    for c in &mut fdm.models.ground_reactions.contacts {
        c.reporting.enabled = on;
    }
}

crate::snapshot::visit_unit_enum! {
    TrimMode { Longitudinal, Full, Ground, Pullup, Custom, Turn, None }
}
