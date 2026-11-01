// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.{h,cpp}.
//
//! The simulation executive (`FGFDMExec`): owns the models, the property
//! tree and the functions, and steps them in JSBSim's order.

mod buoyant_forces;
mod distribute;
mod engine_load;
mod engine_run;
mod exec_bind;
mod ic_load;
mod input;
mod load;
mod models_run;
mod order;
mod output;
mod planet;
mod propulsion;
mod run;
mod script;
mod snapshot;
mod state;
mod trim;

#[cfg(test)]
mod tests;

pub use crate::models::ResetMask;

use crate::Error;
use crate::functions::FunctionSet;
use crate::models::{
    Bindings, Env, Kind, ModelFunctions, Models, accelerations, aircraft, convert_write, propagate,
};
use crate::properties::{NodeId, PropertyTree};

/// The executive.
///
/// `Fdm` is `Send` but not `Sync`: function operands and table lookups
/// keep their state in [`Cell`](std::cell::Cell)s, and a
/// [`GroundCallback`](crate::GroundCallback) need only be `Send`. It moves
/// between threads, and only one thread may reach it at a time; to store
/// it in a `Sync` container, wrap it in a `Mutex<Fdm>` or an
/// exclusive-access `Sync` wrapper.
pub struct Fdm {
    env: Env,
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

impl std::fmt::Debug for Fdm {
    /// A summary: the models and the property tree are far too large to
    /// show whole.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let exec = &self.models.exec;
        f.debug_struct("Fdm")
            .field("aircraft", &self.models.aircraft.name)
            .field("sim_time", &exec.sim_time)
            .field("dt", &exec.dt)
            .field("frame", &exec.frame)
            .field("model_loaded", &self.model_loaded)
            .finish_non_exhaustive()
    }
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
            env: Env {
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
    #[cfg_attr(not(feature = "__oracle"), allow(dead_code))]
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
    /// their getter, function outputs evaluate. A valid path with no node
    /// reads `Ok(0.0)`; [`Fdm::node`] tells whether a node exists.
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
