// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.h (eModels).
//
//! The order in which models run.

/// JSBSim's model order (`eModels`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Model {
    Propagate,
    Input,
    Inertial,
    Atmosphere,
    Winds,
    Systems,
    MassBalance,
    Auxiliary,
    Propulsion,
    Aerodynamics,
    GroundReactions,
    ExternalReactions,
    BuoyantForces,
    Aircraft,
    Accelerations,
    Output,
}

pub(crate) const ORDER: [Model; 16] = [
    Model::Propagate,
    Model::Input,
    Model::Inertial,
    Model::Atmosphere,
    Model::Winds,
    Model::Systems,
    Model::MassBalance,
    Model::Auxiliary,
    Model::Propulsion,
    Model::Aerodynamics,
    Model::GroundReactions,
    Model::ExternalReactions,
    Model::BuoyantForces,
    Model::Aircraft,
    Model::Accelerations,
    Model::Output,
];
