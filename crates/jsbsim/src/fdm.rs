// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.{h,cpp}.
//
//! The simulation executive (`FGFDMExec`): owns the models, the property
//! tree and the functions, and steps them in JSBSim's order.

mod bindings;
mod exec_bind;
mod load;
mod models_run;
mod order;
mod run;
mod script;

#[cfg(test)]
mod tests;

pub use bindings::{Binder, Bindings, Getter, Kind, ModelGetters, Setter, convert_write};
pub use load::{LoadContext, ModelFunctions};
pub use run::InitialState;

use crate::Error;
use crate::functions::{Evaluator, FunctionSet};
use crate::models::accelerations::{self, Accelerations};
use crate::models::aircraft::{self, Aircraft};
use crate::models::atmosphere::Atmosphere;
use crate::models::auxiliary::Auxiliary;
use crate::models::forces::ForceModel;
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
    /// `simulation/jsbsim-debug`.
    pub debug_level: i32,
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
            debug_level: 0,
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
#[derive(Debug, Clone, Default)]
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
    pub propulsion: ForceModel,
    /// `FGAerodynamics`.
    pub aerodynamics: crate::models::aerodynamics::Aerodynamics,
    /// `FGGroundReactions`.
    pub ground_reactions: crate::models::ground_reactions::GroundReactions,
    /// `FGExternalReactions`.
    pub external_reactions: crate::models::external_reactions::ExternalReactions,
    /// `FGBuoyantForces`.
    pub buoyant_forces: ForceModel,
    /// `FGAircraft`.
    pub aircraft: Aircraft,
    /// `FGAccelerations`.
    pub accelerations: Accelerations,
    /// `FGInput`, which has no inputs in the core.
    pub input: ForceModel,
    /// `FGOutput`, which writes no files in the core.
    pub output: ForceModel,
}

/// The executive.
pub struct Fdm {
    tree: PropertyTree,
    functions: FunctionSet,
    bindings: Bindings,
    /// The models.
    pub models: Models,
    initial: InitialState,
    /// Declarations and pre/post functions of each model, by `Model`.
    model_functions: Vec<ModelFunctions>,
    options: crate::LoadOptions,
    script: Option<crate::script::Script>,
    aircraft_path: String,
}

impl Fdm {
    /// `FGFDMExec()` with default load options.
    pub fn new() -> Result<Self, Error> {
        Self::with_options(crate::LoadOptions::default())
    }

    /// `FGFDMExec()`: allocates the models, binds their properties and
    /// initialises them; models load with `options`.
    pub fn with_options(options: crate::LoadOptions) -> Result<Self, Error> {
        let mut fdm = Self {
            tree: PropertyTree::new(),
            functions: FunctionSet::new(),
            bindings: Bindings::default(),
            models: Models {
                fcs: crate::models::fcs::Fcs::new(),
                ..Models::default()
            },
            initial: InitialState::default(),
            model_functions: vec![ModelFunctions::default(); order::ORDER.len()],
            options: crate::LoadOptions::default(),
            script: None,
            aircraft_path: "aircraft".to_owned(),
        };
        fdm.options = options;
        fdm.functions = FunctionSet::with_options(options);
        fdm.bind_all()?;
        fdm.load_planet_constants();
        fdm.initialize_models()?;
        Ok(fdm)
    }

    /// Binds every model in `FGFDMExec::Allocate`'s construction order:
    /// each model's enable flag, then its properties.
    fn bind_all(&mut self) -> Result<(), Error> {
        let b = &mut Binder {
            tree: &mut self.tree,
            bindings: &mut self.bindings,
        };
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
        enable!("FGAtmosphere", atmosphere);
        crate::models::atmosphere::bind(b)?;
        enable!("FGWinds", winds);
        enable!("FGFCS", fcs);
        crate::models::fcs::bind(b)?;
        enable!("FGMassBalance", mass_balance);
        crate::models::mass_balance::bind(b)?;
        enable!("FGAuxiliary", auxiliary);
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
        exec_bind::bind_exec(b)
    }

    /// `FGMassBalance::Load(<mass_balance>)`, tying the point mass
    /// properties as it adds them.
    pub fn load_mass_balance(&mut self, el: &crate::xml::Element) -> Result<(), Error> {
        let b = &mut Binder {
            tree: &mut self.tree,
            bindings: &mut self.bindings,
        };
        self.models.mass_balance.load(el, b)
    }

    /// `SetAircraftPath`: where `load_script` finds aircraft, relative to
    /// the resolver's root.
    pub fn set_aircraft_path(&mut self, path: &str) {
        self.aircraft_path = path.to_owned();
    }

    /// The property tree.
    #[must_use]
    pub fn tree(&self) -> &PropertyTree {
        &self.tree
    }

    /// The node at `path`, if it exists.
    pub fn node(&self, path: &str) -> Result<Option<NodeId>, Error> {
        Ok(self.tree.get(path)?)
    }

    /// Reads `path` as JSBSim's `GetPropertyValue` does: tied nodes call
    /// their getter, function outputs evaluate.
    pub fn get_property(&mut self, path: &str) -> Result<f64, Error> {
        let Some(node) = self.tree.get(path)? else {
            return Ok(0.0);
        };
        self.read_node(node)
    }

    /// Reads one node through getters and functions.
    pub fn read_node(&mut self, node: NodeId) -> Result<f64, Error> {
        let getters = ModelGetters {
            bindings: &self.bindings,
            models: &self.models,
        };
        Evaluator::with_getters(&self.functions, &mut self.tree, &getters).read_node(node)
    }

    /// Writes `path` as `SetPropertyValue` does: a node tied to a setter
    /// calls it, a read-only node ignores the write, others store the value.
    /// The node is created when missing.
    pub fn set_property(&mut self, path: &str, value: f64) -> Result<bool, Error> {
        let node = self.tree.get_or_create(self.tree.root(), path)?;
        self.write_node(node, value)
    }

    /// Writes one node; false when it, or a node it aliases, is not
    /// writable. Setter requests that need the executive are then run.
    pub fn write_node(&mut self, node: NodeId, value: f64) -> Result<bool, Error> {
        let Some(target) = self.tree.write_target(node) else {
            return Ok(false);
        };
        let written = match self.bindings.setter(target) {
            Some((kind, setter)) => {
                setter(&mut self.models, convert_write(*kind, value));
                true
            }
            None => self.tree.set_double(node, value),
        };
        self.process_requests()?;
        Ok(written)
    }
}
