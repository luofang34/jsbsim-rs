// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.h (the property manager the models
// share).
//
//! The evaluation environment: the property tree, the functions and the
//! tied-property bindings.
//!
//! It is a part of [`Fdm`](super::Fdm) separate from the models, so model
//! code can evaluate functions against `&Models` while it holds borrows of
//! its own model's load-time data, then apply the results.

use super::{Binder, Bindings, LoadContext, ModelGetters, Models};
use crate::Error;
use crate::functions::{Evaluator, FnId, FunctionSet};
use crate::properties::PropertyTree;

/// What evaluates properties and functions against the models.
pub(crate) struct Env {
    pub(crate) tree: PropertyTree,
    pub(crate) functions: FunctionSet,
    pub(crate) bindings: Bindings,
}

impl Env {
    /// Runs `f` with one evaluator whose getters read `models`.
    pub(crate) fn eval<R>(
        &mut self,
        models: &Models,
        f: impl FnOnce(&mut Evaluator<'_>) -> Result<R, Error>,
    ) -> Result<R, Error> {
        let getters = ModelGetters {
            bindings: &self.bindings,
            models,
        };
        f(&mut Evaluator::with_getters(
            &self.functions,
            &mut self.tree,
            &getters,
        ))
    }

    /// `cacheValue(true)` of function `id`.
    pub(crate) fn cache_value(&mut self, models: &Models, id: FnId) -> Result<(), Error> {
        let getters = ModelGetters {
            bindings: &self.bindings,
            models,
        };
        self.functions
            .cache_value_with(id, true, &mut self.tree, &getters)
    }

    /// `cacheValue(true)` followed by `GetValue()`.
    pub(crate) fn cached_value(&mut self, models: &Models, id: FnId) -> Result<f64, Error> {
        self.cache_value(models, id)?;
        Ok(self.functions.function(id)?.cached().unwrap_or(f64::NAN))
    }

    /// Ties properties to model getters and setters.
    pub(crate) fn binder(&mut self) -> Binder<'_> {
        Binder {
            tree: &mut self.tree,
            bindings: &mut self.bindings,
        }
    }

    /// What a model's `Load` may touch besides the model itself.
    pub(crate) fn load_context(&mut self, options: crate::LoadOptions) -> LoadContext<'_> {
        LoadContext {
            functions: &mut self.functions,
            binder: Binder {
                tree: &mut self.tree,
                bindings: &mut self.bindings,
            },
            options,
            ic_position: None,
        }
    }
}
