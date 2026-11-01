// SPDX-License-Identifier: LGPL-2.1-or-later
//! Simulation models (atmosphere, propagation, aerodynamics, FCS, propulsion
//! and the rest of JSBSim's `FGModel` family).

mod bindings;
mod env;
mod exec;
mod load;
mod reset_mask;

pub mod accelerations;
pub mod aerodynamics;
pub mod aircraft;
pub mod atmosphere;
pub mod auxiliary;
pub mod base;
pub mod buoyant_forces;
pub mod external_reactions;
pub mod fcs;
pub mod ground_reactions;
pub mod inertial;
pub mod input;
pub mod mass_balance;
pub mod output;
pub mod propagate;
pub mod propulsion;
pub mod winds;

pub(crate) use bindings::{Binder, Bindings, Kind, ModelEvaluator, ModelGetters, convert_write};
pub(crate) use env::Env;
pub(crate) use exec::{ExecRequest, ExecState, Models};
pub(crate) use load::{LoadContext, ModelFunctions};
pub use reset_mask::ResetMask;
