// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.{h,cpp} (the state the executive keeps
// and the models it owns).
//
//! Every model and the executive state, in one place so getters can read any
//! of them as JSBSim's getters do.

use super::ResetMask;
use super::accelerations::Accelerations;
use super::aircraft::Aircraft;
use super::atmosphere::Atmosphere;
use super::auxiliary::Auxiliary;
use super::inertial::Inertial;
use super::mass_balance::MassBalance;
use super::propagate::Propagate;
use super::winds::Winds;

/// Requests from setters that need the whole executive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecRequest {
    /// `simulation/reset` with its mode bits, unnamed ones included.
    Reset(ResetMask),
    /// `simulation/do_simple_trim` with its mode, which may be out of
    /// range: the executive reports it as JSBSim's illegal trim.
    Trim(i32),
    /// `simulation/do_linearization`.
    Linearize,
    /// `propulsion/set-running` with the engine, -1 for all.
    InitRunning(i32),
}

/// Executive state that models and getters read.
#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "__oracle"), allow(unreachable_pub))]
pub struct ExecState {
    /// Simulation time, s.
    pub sim_time: f64,
    /// Time step, s; 0 while integration is suspended.
    pub dt: f64,
    /// Time step saved while integration is suspended.
    pub saved_dt: f64,
    /// Frames run.
    pub frame: u32,
    /// `simulation/pause`.
    pub holding: bool,
    /// `simulation/terminate`.
    pub terminate: bool,
    /// `forces/hold-down`.
    pub hold_down: bool,
    /// Whether a trim is in progress.
    pub trim_status: bool,
    /// `simulation/trim-completed`.
    pub trim_completed: i32,
    /// `simulation/randomseed`.
    pub random_seed: i32,
    /// `RandomGenerator`, seeded with [`random_seed`](Self::random_seed):
    /// the generator of every sensor, function and wind model without a
    /// seed of its own.
    pub rng: crate::random::SharedGenerator,
    /// `simulation/jsbsim-debug`.
    pub debug_level: i32,
    /// Seconds since the Unix epoch, UTC, standing for the wall clock
    /// (`time()`) JSBSim reads; `FGMagnetometer` dates its field model by
    /// it when it loads. The core reads no clock: set it before loading.
    pub wall_clock: i64,
    /// Setter requests the executive still has to carry out.
    pub(crate) requests: Vec<ExecRequest>,
}

impl Default for ExecState {
    fn default() -> Self {
        Self {
            sim_time: 0.0,
            dt: 1.0 / 120.0,
            saved_dt: 0.0,
            frame: 0,
            holding: false,
            terminate: false,
            hold_down: false,
            trim_status: false,
            trim_completed: 0,
            random_seed: 0,
            rng: crate::random::SharedGenerator::default(),
            debug_level: 0,
            wall_clock: 0,
            requests: Vec::new(),
        }
    }
}

impl ExecState {
    /// `IntegrationSuspended`.
    #[must_use]
    pub(crate) fn integration_suspended(&self) -> bool {
        self.dt == 0.0
    }
}

/// Every model, plus the executive state, in one place so getters can
/// read any of them as JSBSim's getters do.
#[derive(Debug, Default)]
#[cfg_attr(not(feature = "__oracle"), allow(unreachable_pub))]
pub struct Models {
    /// Executive state.
    pub exec: ExecState,
    /// `FGInertial`.
    pub inertial: Inertial,
    /// `FGPropagate`.
    pub propagate: Propagate,
    /// `FGAtmosphere`.
    pub atmosphere: Atmosphere,
    /// `FGWinds`.
    pub winds: Winds,
    /// `FGFCS`.
    pub fcs: crate::models::fcs::Fcs,
    /// `FGMassBalance`.
    pub mass_balance: MassBalance,
    /// `FGAuxiliary`.
    pub auxiliary: Auxiliary,
    /// `FGPropulsion`.
    pub propulsion: crate::models::propulsion::Propulsion,
    /// `FGAerodynamics`.
    pub aerodynamics: crate::models::aerodynamics::Aerodynamics,
    /// `FGGroundReactions`.
    pub ground_reactions: crate::models::ground_reactions::GroundReactions,
    /// `FGExternalReactions`.
    pub external_reactions: crate::models::external_reactions::ExternalReactions,
    /// `FGBuoyantForces`.
    pub buoyant_forces: crate::models::buoyant_forces::BuoyantForces,
    /// `FGAircraft`.
    pub aircraft: Aircraft,
    /// `FGAccelerations`.
    pub accelerations: Accelerations,
    /// `FGInput`, which opens no sockets in the core.
    pub input: crate::models::input::Input,
    /// `FGOutput`, which writes no files in the core.
    pub output: crate::models::output::Output,
    /// `FGInitialCondition`, which `run_ic` applies.
    pub ic: crate::initial_condition::InitialCondition,
}
