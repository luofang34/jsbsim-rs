// SPDX-License-Identifier: LGPL-2.1-or-later
//! Simulation models (atmosphere, propagation, aerodynamics, FCS, propulsion
//! and the rest of JSBSim's `FGModel` family).

mod bindings;
mod env;
mod exec;
mod load;
mod reset_mask;

pub(crate) mod accelerations;
pub(crate) mod aerodynamics;
pub(crate) mod aircraft;
pub(crate) mod atmosphere;
pub(crate) mod auxiliary;
pub(crate) mod base;
pub(crate) mod buoyant_forces;
pub(crate) mod external_reactions;
pub(crate) mod fcs;
pub(crate) mod ground_reactions;
pub(crate) mod inertial;
pub(crate) mod input;
pub(crate) mod mass_balance;
pub(crate) mod output;
pub(crate) mod propagate;
pub(crate) mod propulsion;
pub(crate) mod winds;

pub(crate) use bindings::{Binder, Bindings, Kind, ModelEvaluator, ModelGetters, convert_write};
pub(crate) use env::Env;
pub(crate) use exec::{ExecRequest, ExecState, Models};
pub(crate) use load::{LoadContext, ModelFunctions};
pub use reset_mask::ResetMask;
