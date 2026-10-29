// SPDX-License-Identifier: LGPL-2.1-or-later
// FGFunction: Copyright (C) 2004 Jon S. Berndt (jon@jsbsim.org).
// FGTemplateFunc: Copyright (C) 2018 B. Coconnier (bcoconni@users.sf.net).
// Derived from JSBSim src/math/FGFunction.cpp (output binding),
// FGTemplateFunc.{h,cpp} and string_utilities.cpp (replace).
//
//! Tables, functions and conditions (`FGTable`, `FGFunction`,
//! `FGCondition` and their parameters).
//!
//! JSBSim ties the property of a named function or table to a getter, so
//! every read of that property evaluates it. Here all functions and tables
//! live in a [`FunctionSet`] that maps such properties to their owner, and
//! every read goes through an [`Evaluator`], which evaluates bound
//! properties on the spot and stores the result in the tree.

mod condition;
mod error;
mod evaluator;
mod load;
mod ops;
mod parameter;
mod random;
mod table;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

pub use condition::Condition;
pub use error::FunctionError;
pub use evaluator::{Evaluator, GetterSource, NoGetters};
pub use ops::{OPERATIONS, Op};
pub use parameter::{Parameter, PropertyValue};
pub use table::{Table, TableKind};

use crate::LoadOptions;
use crate::properties::{Attribute, NodeId, NodeMap, PropertyTree, Value, mk_property_name};
use crate::xml::{Location, atof_locale_c};

/// Index of a function in a [`FunctionSet`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FnId(usize);

/// Index of a table in a [`FunctionSet`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TableId(usize);

/// What a bound property reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bound {
    Function(FnId),
    Table(TableId),
}

/// One operation with its operands.
#[derive(Debug, Clone)]
pub struct Function {
    name: String,
    op: Op,
    params: Vec<Parameter>,
    node: Option<NodeId>,
    copy_to: Option<NodeId>,
    cached: Option<f64>,
    at: Location,
    /// The draw of a `<random>` or `<urandom>`.
    random: Option<random::RandomDraw>,
}

impl Function {
    /// The `name` attribute (after `#` substitution).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The operation.
    #[must_use]
    pub fn op(&self) -> Op {
        self.op
    }

    /// Output property, if bound.
    #[must_use]
    pub fn node(&self) -> Option<NodeId> {
        self.node
    }

    /// The value cached by [`FunctionSet::cache_value`], if any.
    #[must_use]
    pub fn cached(&self) -> Option<f64> {
        self.cached
    }
}

/// All functions, tables and templates of a simulation.
#[derive(Debug, Clone, Default)]
pub struct FunctionSet {
    functions: Vec<Function>,
    tables: Vec<Table>,
    bound: NodeMap<Bound>,
    templates: BTreeMap<String, FnId>,
    options: LoadOptions,
    /// The executive's generator, which `<random>` and `<urandom>` without
    /// a `seed` draw from; a set of its own until the executive shares
    /// its generator ([`set_exec_generator`](Self::set_exec_generator)).
    exec_rng: crate::random::SharedGenerator,
}

impl FunctionSet {
    /// An empty set with default (JSBSim-compatible) options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty set loading with `options`.
    #[must_use]
    pub fn with_options(options: LoadOptions) -> Self {
        Self {
            options,
            ..Self::default()
        }
    }

    /// Makes `<random>` and `<urandom>` without a `seed` draw from the
    /// executive's generator `rng`, as `FGFDMExec::GetRandomGenerator`
    /// shares it.
    pub fn set_exec_generator(&mut self, rng: crate::random::SharedGenerator) {
        self.exec_rng = rng;
    }

    /// Something JSBSim logs and ignores: logged, or refused when strict.
    fn lenient(&self, at: &Location, message: String) -> Result<(), crate::Error> {
        if self.options.strict {
            return Err(FunctionError::Strict {
                at: at.clone(),
                message,
            }
            .into());
        }
        tracing::error!("{at}: {message}");
        Ok(())
    }

    /// The function `id`; an error only for an id another set created.
    pub fn function(&self, id: FnId) -> Result<&Function, FunctionError> {
        match self.functions.get(id.0) {
            Some(function) => Ok(function),
            None => Err(unknown_id("function", id.0)),
        }
    }

    fn function_mut(&mut self, id: FnId) -> Result<&mut Function, FunctionError> {
        match self.functions.get_mut(id.0) {
            Some(function) => Ok(function),
            None => Err(unknown_id("function", id.0)),
        }
    }

    /// The table `id`; an error only for an id another set created.
    pub fn table(&self, id: TableId) -> Result<&Table, FunctionError> {
        match self.tables.get(id.0) {
            Some(table) => Ok(table),
            None => Err(unknown_id("table", id.0)),
        }
    }

    /// The template registered as `name` (`GetTemplateFunc`).
    #[must_use]
    pub fn template(&self, name: &str) -> Option<FnId> {
        self.templates.get(name).copied()
    }

    /// `cacheValue(cache)`: with `cache`, evaluates `id` now and returns
    /// that value until called again; without, evaluates on every read.
    pub fn cache_value(
        &mut self,
        id: FnId,
        cache: bool,
        tree: &mut PropertyTree,
    ) -> Result<(), crate::Error> {
        self.cache_value_with(id, cache, tree, NoGetters)
    }

    /// [`cache_value`](Self::cache_value) reading model getters.
    pub fn cache_value_with(
        &mut self,
        id: FnId,
        cache: bool,
        tree: &mut PropertyTree,
        getters: impl GetterSource,
    ) -> Result<(), crate::Error> {
        self.function_mut(id)?.cached = None;
        if cache {
            let value = Evaluator::with_getters(self, tree, getters).function(id)?;
            self.function_mut(id)?.cached = Some(value);
        }
        Ok(())
    }

    fn push_function(&mut self, function: Function) -> FnId {
        self.functions.push(function);
        FnId(self.functions.len() - 1)
    }

    fn push_table(&mut self, table: Table) -> TableId {
        self.tables.push(table);
        TableId(self.tables.len() - 1)
    }

    /// Ties `property` to `what` as JSBSim's getter-only `Tie` does. Fails
    /// when the node is already tied; an alias cannot be tied, which JSBSim
    /// only logs.
    fn bind(
        &mut self,
        tree: &mut PropertyTree,
        property: &str,
        what: Bound,
        at: &Location,
    ) -> Result<NodeId, crate::Error> {
        let node = tree.get_or_create(tree.root(), property)?;
        if tree.is_tied(node) {
            return Err(FunctionError::AlreadyBound {
                property: property.to_owned(),
                at: at.clone(),
            }
            .into());
        }
        if tree.tie(node, Value::Double(0.0), false) {
            self.bound.insert(node, what);
        } else {
            self.lenient(
                at,
                format!("Failed to tie property {property} to object methods"),
            )?;
        }
        Ok(node)
    }

    /// Replaces a constant function's binding by its value: the node is
    /// untied, holds `value` and becomes read-only.
    fn unbind_as_constant(&mut self, tree: &mut PropertyTree, node: NodeId, value: f64) {
        if self.bound.remove(node).is_some() {
            tree.untie(node);
        }
        tree.set_double(node, value);
        tree.set_attribute(node, Attribute::WRITE, false);
    }
}

/// The error of an id lookup that missed. The id accessors run several
/// times per frame, so the error is built only on the miss path: an eager
/// `ok_or(FunctionError::..)` costs a `FunctionError` drop on every hit.
#[cold]
fn unknown_id(what: &'static str, id: usize) -> FunctionError {
    FunctionError::UnknownId { what, id }
}

/// `is_number`: whether `text` passes `atof_locale_c`.
#[must_use]
pub fn is_number(text: &str) -> bool {
    atof_locale_c(text).is_ok()
}

/// JSBSim's `replace(text, "#", prefix)` when `prefix` is a number: the
/// first `#` only. Other prefixes leave `text` unchanged.
#[must_use]
pub fn substitute_hash(text: &str, prefix: &str) -> String {
    if is_number(prefix) {
        text.replacen('#', prefix, 1)
    } else {
        text.to_owned()
    }
}

/// `CreateOutputNode`'s property name for `name` under `prefix`;
/// `Err(message)` when JSBSim would log an error and leave the output
/// unbound.
fn output_name(name: &str, prefix: &str) -> Result<Option<String>, String> {
    if name.is_empty() {
        return Ok(None);
    }
    if prefix.is_empty() {
        return Ok(Some(mk_property_name(name, false)));
    }
    if !is_number(prefix) {
        return Ok(Some(mk_property_name(&format!("{prefix}/{name}"), false)));
    }
    if name.contains('#') {
        return Ok(Some(mk_property_name(
            &name.replacen('#', prefix, 1),
            false,
        )));
    }
    Err(format!(
        "Malformed function name with number: {prefix} and property name: {name} but no \"#\" sign for substitution."
    ))
}
