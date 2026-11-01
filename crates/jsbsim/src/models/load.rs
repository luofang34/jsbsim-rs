// SPDX-License-Identifier: LGPL-2.1-or-later
// FGModelFunctions: Copyright (C) 2010 Jon S. Berndt (jon@jsbsim.org).
// FGPropertyReader: Copyright (C) 2014 Bertrand Coconnier.
// Derived from JSBSim src/math/FGModelFunctions.h and
// src/input_output/FGPropertyReader.h (what they keep).
//
//! What a model's `Load` works with, and what it keeps of its `<property>`
//! declarations and functions.

use super::Binder;
use crate::LoadOptions;
use crate::functions::{FnId, FunctionSet};
use crate::properties::{NodeMap, PropertyTree};

/// What a model's `Load` may touch besides the model itself: the tree
/// (through the binder), the functions and the options.
pub(crate) struct LoadContext<'a> {
    /// Functions and tables.
    pub functions: &'a mut FunctionSet,
    /// Tied-property bindings and the property tree.
    pub binder: Binder<'a>,
    /// Load options.
    pub options: LoadOptions,
    /// The initial condition's position (`GetIC()->GetPosition()`), while
    /// FCS channels load.
    pub ic_position: Option<crate::math::Location>,
}

impl LoadContext<'_> {
    /// The property tree.
    pub(crate) fn tree(&mut self) -> &mut PropertyTree {
        self.binder.tree
    }
}

/// `FGModelFunctions` plus `FGPropertyReader`: a model's `<property>`
/// declarations and its pre- and post-functions.
#[derive(Debug, Clone, Default)]
pub(crate) struct ModelFunctions {
    /// Functions run (and cached) before the model.
    pub pre: Vec<FnId>,
    /// Functions run (and cached) after the model.
    pub post: Vec<FnId>,
    /// Declared properties and the value `InitModel` restores.
    pub local_properties: NodeMap<f64>,
}
