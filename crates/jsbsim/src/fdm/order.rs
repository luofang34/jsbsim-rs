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

/// One `T` per model, indexed by [`Model`] so no slot can be missing.
#[derive(Debug, Clone, Default)]
pub(crate) struct PerModel<T>([T; ORDER.len()]);

/// The slot of `$model` in the array `$slots` (a reference to it).
macro_rules! per_model_slot {
    ($slots:expr, $model:expr) => {{
        let [
            propagate,
            input,
            inertial,
            atmosphere,
            winds,
            systems,
            mass_balance,
            auxiliary,
            propulsion,
            aerodynamics,
            ground_reactions,
            external_reactions,
            buoyant_forces,
            aircraft,
            accelerations,
            output,
        ] = $slots;
        match $model {
            Model::Propagate => propagate,
            Model::Input => input,
            Model::Inertial => inertial,
            Model::Atmosphere => atmosphere,
            Model::Winds => winds,
            Model::Systems => systems,
            Model::MassBalance => mass_balance,
            Model::Auxiliary => auxiliary,
            Model::Propulsion => propulsion,
            Model::Aerodynamics => aerodynamics,
            Model::GroundReactions => ground_reactions,
            Model::ExternalReactions => external_reactions,
            Model::BuoyantForces => buoyant_forces,
            Model::Aircraft => aircraft,
            Model::Accelerations => accelerations,
            Model::Output => output,
        }
    }};
}

impl<T> std::ops::Index<Model> for PerModel<T> {
    type Output = T;
    fn index(&self, model: Model) -> &T {
        per_model_slot!(&self.0, model)
    }
}

impl<T> std::ops::IndexMut<Model> for PerModel<T> {
    fn index_mut(&mut self, model: Model) -> &mut T {
        per_model_slot!(&mut self.0, model)
    }
}
