// SPDX-License-Identifier: LGPL-2.1-or-later
//! Simulation models (atmosphere, propagation, aerodynamics, FCS, propulsion
//! and the rest of JSBSim's `FGModel` family).

pub mod accelerations;
pub mod aerodynamics;
pub mod aircraft;
pub mod atmosphere;
pub mod auxiliary;
pub mod base;
pub mod external_reactions;
pub mod fcs;
pub mod forces;
pub mod ground_reactions;
pub mod inertial;
pub mod mass_balance;
pub mod propagate;
pub mod propulsion;
pub mod winds;
