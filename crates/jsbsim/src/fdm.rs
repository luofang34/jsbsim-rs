// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.{h,cpp}.
//
//! The simulation executive (`FGFDMExec`): owns the models, the property
//! tree and the functions, and steps them in JSBSim's order.

mod bindings;
mod distribute;
mod engine_load;
mod engine_run;
pub use engine_load::EngineLoad;
mod env;
pub(crate) use env::Env;
mod buoyancy;
mod exec_bind;
mod ic_load;
mod input;
mod load;
mod models_run;
mod order;
mod planet;
mod propulsion;
mod reset_mask;
mod run;
mod script;
mod snapshot;
mod state;
mod trim;

#[cfg(test)]
mod tests;

pub use bindings::{Binder, Bindings, Kind, ModelEvaluator, ModelGetters, convert_write};
pub use exec_bind::ExecRequest;
pub use load::{LoadContext, ModelFunctions};
pub use reset_mask::ResetMask;
pub use run::InitialState;

use crate::Error;
use crate::functions::FunctionSet;
use crate::models::accelerations::{self, Accelerations};
use crate::models::aircraft::{self, Aircraft};
use crate::models::atmosphere::Atmosphere;
use crate::models::auxiliary::Auxiliary;
use crate::models::inertial::Inertial;
use crate::models::mass_balance::MassBalance;
use crate::models::propagate::{self, Propagate};
use crate::models::winds::Winds;
use crate::properties::{NodeId, PropertyTree};

/// Executive state that models and getters read.
#[derive(Debug, Clone)]
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
    pub requests: Vec<exec_bind::ExecRequest>,
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
    pub fn integration_suspended(&self) -> bool {
        self.dt == 0.0
    }
}

/// Every model, plus the executive state, in one place so getters can
/// read any of them as JSBSim's getters do.
#[derive(Debug, Default)]
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

/// The executive.
pub struct Fdm {
    env: env::Env,
    /// The models.
    #[cfg(feature = "__oracle")]
    pub models: Models,
    /// The models.
    #[cfg(not(feature = "__oracle"))]
    pub(crate) models: Models,
    /// Declarations and pre/post functions of each model, by `Model`.
    model_functions: order::PerModel<ModelFunctions>,
    options: crate::LoadOptions,
    script: Option<crate::script::Script>,
    aircraft_path: String,
    /// `FullAircraftPath`: the loaded aircraft's directory, where model
    /// includes are found.
    full_aircraft_path: String,
    systems_path: String,
    engine_path: String,
    /// Whether a model document has begun loading, after which another
    /// load would mix two aircraft.
    model_loaded: bool,
    last_trim: Option<crate::trim::TrimReport>,
    trims_run: u32,
    pub(crate) last_linearization: Option<crate::linearization::Linearization>,
    /// What the loads so far contribute to a snapshot's fingerprint.
    snapshot_record: crate::snapshot::LoadRecord,
}

impl Fdm {
    /// `FGFDMExec()` with default load options.
    ///
    /// # Errors
    ///
    /// As [`with_options`](Self::with_options).
    pub fn new() -> Result<Self, Error> {
        Self::with_options(crate::LoadOptions::default())
    }

    /// `FGFDMExec()`: allocates the models, binds their properties and
    /// initialises them; models load with `options`.
    ///
    /// # Errors
    ///
    /// [`Error::PropertyPath`] or [`Error::Function`] if the built-in
    /// models cannot bind or initialise their properties. Neither depends
    /// on any input, so either one is a bug in the port.
    pub fn with_options(options: crate::LoadOptions) -> Result<Self, Error> {
        let mut fdm = Self {
            env: env::Env {
                tree: PropertyTree::for_instance(0),
                functions: FunctionSet::new(),
                bindings: Bindings::default(),
            },
            models: Models {
                fcs: crate::models::fcs::Fcs::new(),
                ..Models::default()
            },
            model_functions: order::PerModel::default(),
            options: crate::LoadOptions::default(),
            script: None,
            aircraft_path: "aircraft".to_owned(),
            full_aircraft_path: String::new(),
            systems_path: "systems".to_owned(),
            engine_path: "engine".to_owned(),
            model_loaded: false,
            last_trim: None,
            trims_run: 0,
            last_linearization: None,
            snapshot_record: crate::snapshot::LoadRecord::default(),
        };
        fdm.options = options;
        fdm.env.functions = FunctionSet::with_options(options);
        fdm.env
            .functions
            .set_exec_generator(fdm.models.exec.rng.clone());
        fdm.bind_all()?;
        fdm.load_planet_constants();
        fdm.initialize_models()?;
        fdm.models.ic.initialize_ic(&fdm.models.inertial);
        fdm.loaded_without_documents();
        Ok(fdm)
    }

    /// Binds every model in `FGFDMExec::Allocate`'s construction order
    /// (each model's enable flag, then its properties), then the initial
    /// conditions, then the executive.
    fn bind_all(&mut self) -> Result<(), Error> {
        let b = &mut self.env.binder();
        macro_rules! enable {
            ($name:literal, $field:ident) => {
                exec_bind::bind_enable(b, $name, |m| &m.$field.base, |m| &mut m.$field.base)?
            };
        }
        enable!("FGInertial", inertial);
        crate::models::inertial::bind(b)?;
        enable!("FGPropagate", propagate);
        propagate::bind(b)?;
        enable!("FGInput", input);
        exec_bind::bind_atmosphere_enable(b)?;
        crate::models::atmosphere::bind(b)?;
        enable!("FGWinds", winds);
        crate::models::winds::bind(b)?;
        enable!("FGFCS", fcs);
        crate::models::fcs::bind(b)?;
        enable!("FGMassBalance", mass_balance);
        crate::models::mass_balance::bind(b)?;
        enable!("FGAuxiliary", auxiliary);
        crate::models::auxiliary::bind(b)?;
        enable!("FGPropulsion", propulsion);
        enable!("FGAerodynamics", aerodynamics);
        crate::models::aerodynamics::bind(b)?;
        enable!("FGGroundReactions", ground_reactions);
        crate::models::ground_reactions::bind(b)?;
        enable!("FGExternalReactions", external_reactions);
        crate::models::external_reactions::bind(b)?;
        enable!("FGBuoyantForces", buoyant_forces);
        enable!("FGAircraft", aircraft);
        aircraft::bind(b)?;
        enable!("FGAccelerations", accelerations);
        accelerations::bind(b)?;
        enable!("FGOutput", output);
        // `FGOutput::ForceOutput(idx)` prints output type `idx`; the port
        // prints nothing, so a write has no effect.
        b.setter_only("simulation/force-output", Kind::Int, |_, _| {})?;
        crate::initial_condition::bind(b)?;
        exec_bind::bind_exec(b)
    }

    /// `FGMassBalance::Load(<mass_balance>)`, tying the point mass
    /// properties as it adds them.
    pub(crate) fn load_mass_balance(&mut self, el: &crate::xml::Element) -> Result<(), Error> {
        let loaded = self.models.mass_balance.load(el, &mut self.env.binder());
        self.loaded_without_documents();
        loaded
    }

    /// `SetAircraftPath`: where `load_script` finds aircraft, relative to
    /// the resolver's root.
    pub fn set_aircraft_path(&mut self, path: &str) {
        self.aircraft_path = path.to_owned();
    }

    /// `GetInertial()->SetGroundCallback(gc)`: every terrain query from
    /// now on goes to `callback`. Install it before loading a script or
    /// model so that initial conditions given above ground are placed on
    /// its terrain. It stays installed across resets and planet loads.
    pub fn set_ground_callback(
        &mut self,
        callback: Box<dyn crate::models::inertial::GroundCallback>,
    ) {
        self.models.inertial.set_ground_callback(callback);
    }

    /// `SetSystemsPath`: where `<system file="...">` includes are found
    /// after the aircraft's own directories (the aircraft directory, then
    /// its `Systems` and `systems`), relative to the resolver's root.
    pub fn set_systems_path(&mut self, path: &str) {
        self.systems_path = path.to_owned();
    }

    /// `SetEnginePath`: where `<engine file="...">` and
    /// `<thruster file="...">` includes are found after the aircraft's
    /// `Engines`, `engines`, `Engine` and `engine` directories (not the
    /// aircraft directory itself), relative to the resolver's root.
    pub fn set_engine_path(&mut self, path: &str) {
        self.engine_path = path.to_owned();
    }

    /// The property tree.
    #[must_use]
    pub(crate) fn tree(&self) -> &PropertyTree {
        &self.env.tree
    }

    /// The node at `path`, if it exists.
    ///
    /// # Errors
    ///
    /// [`Error::PropertyPath`] when `path` is not a valid property path.
    pub fn node(&self, path: &str) -> Result<Option<NodeId>, Error> {
        Ok(self.env.tree.get(path)?)
    }

    /// Reads `path` as JSBSim's `GetPropertyValue` does: tied nodes call
    /// their getter, function outputs evaluate.
    ///
    /// # Errors
    ///
    /// [`Error::PropertyPath`] when `path` is not a valid property path,
    /// and [`Error::Function`] when the function bound to it fails to
    /// evaluate.
    pub fn get_property(&mut self, path: &str) -> Result<f64, Error> {
        let Some(node) = self.env.tree.get(path)? else {
            return Ok(0.0);
        };
        self.read_node(node)
    }

    /// Reads one node through getters and functions.
    ///
    /// # Errors
    ///
    /// [`Error::Function`] when the function bound to `node` fails to
    /// evaluate.
    pub fn read_node(&mut self, node: NodeId) -> Result<f64, Error> {
        self.env.eval(&self.models, |ev| ev.read_node(node))
    }

    /// Writes `path` as `SetPropertyValue` does: a node tied to a setter
    /// calls it, a read-only node ignores the write, others store the value.
    /// The node is created when missing.
    ///
    /// # Errors
    ///
    /// [`Error::PropertyPath`] when `path` is not a valid property path;
    /// otherwise as [`write_node`](Self::write_node).
    pub fn set_property(&mut self, path: &str, value: f64) -> Result<bool, Error> {
        let node = self.env.tree.get_or_create(self.env.tree.root(), path)?;
        self.write_node(node, value)
    }

    /// Writes one node; false when it, or a node it aliases, is not
    /// writable. Setter requests that need the executive are then run.
    ///
    /// # Errors
    ///
    /// Errors of the request the write starts: a trim
    /// (`simulation/do_simple_trim`, [`Error::TrimFailed`] when it does
    /// not converge), a reset or initial-condition run
    /// (`simulation/reset`, as [`run_ic`](Self::run_ic)), or a
    /// linearization; and [`Error::Function`] when a function the request
    /// evaluates fails.
    pub fn write_node(&mut self, node: NodeId, value: f64) -> Result<bool, Error> {
        let Some(target) = self.env.tree.write_target(node) else {
            return Ok(false);
        };
        let written = match self.env.bindings.setter(target) {
            Some((kind, setter)) => {
                setter(&mut self.models, convert_write(*kind, value));
                true
            }
            None => {
                self.env.tree.set_double_at_target(target, value);
                true
            }
        };
        self.process_requests()?;
        Ok(written)
    }
}
