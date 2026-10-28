// SPDX-License-Identifier: LGPL-2.1-or-later
// FGFunction: Copyright (C) 2004 Jon S. Berndt (jon@jsbsim.org).
// FGTable, FGPropertyValue: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/math/FGFunction.cpp, FGTable.cpp and
// FGPropertyValue.cpp (GetValue) and src/simgear/props/props.cxx
// (getDoubleValue of tied nodes).
//
//! Evaluation of parameters, functions and tables against a property tree.

mod apply;

use super::{
    Bound, FnId, FunctionError, FunctionSet, Parameter, PropertyValue, Table, TableId, TableKind,
};
use crate::Error;
use crate::properties::{NodeId, PropertyTree};

/// Deeper nesting than this means a property reads itself.
const MAX_DEPTH: u32 = 256;

/// Values of nodes tied to model getters, which JSBSim evaluates on every
/// read of the node.
pub trait GetterSource {
    /// The getter's value if `node` is tied to one.
    fn get(&self, node: NodeId) -> Option<f64>;
}

/// A source with no getters, for function sets used on their own.
pub struct NoGetters;

impl GetterSource for NoGetters {
    fn get(&self, _node: NodeId) -> Option<f64> {
        None
    }
}

/// Reads properties and evaluates functions and tables.
pub struct Evaluator<'a> {
    set: &'a FunctionSet,
    tree: &'a mut PropertyTree,
    getters: &'a dyn GetterSource,
    depth: u32,
    template_argument: Option<NodeId>,
}

impl<'a> Evaluator<'a> {
    /// An evaluator over `set` and `tree`, without model getters.
    pub fn new(set: &'a FunctionSet, tree: &'a mut PropertyTree) -> Self {
        Self::with_getters(set, tree, &NoGetters)
    }

    /// An evaluator that also reads model getters from `getters`.
    pub fn with_getters(
        set: &'a FunctionSet,
        tree: &'a mut PropertyTree,
        getters: &'a dyn GetterSource,
    ) -> Self {
        Self {
            set,
            tree,
            getters,
            depth: 0,
            template_argument: None,
        }
    }

    /// The tree being read.
    pub fn tree(&mut self) -> &mut PropertyTree {
        self.tree
    }

    /// `getDoubleValue()` of `node`: aliases are followed, an unreadable
    /// node reads 0, and a node bound to a function or table evaluates it
    /// and stores the result.
    pub fn read_node(&mut self, node: NodeId) -> Result<f64, Error> {
        let Some(target) = self.tree.read_target(node) else {
            return Ok(0.0);
        };
        let Some(bound) = self.set.bound.get(target).copied() else {
            if let Some(value) = self.getters.get(target) {
                return Ok(value);
            }
            return Ok(self.tree.target_double(target));
        };
        let value = self.nested(target, |e| match bound {
            Bound::Function(id) => e.function(id),
            Bound::Table(id) => e.table(id),
        })?;
        self.tree.store(target, value);
        Ok(value)
    }

    fn nested(
        &mut self,
        node: NodeId,
        f: impl FnOnce(&mut Self) -> Result<f64, Error>,
    ) -> Result<f64, Error> {
        if self.depth >= MAX_DEPTH {
            return Err(FunctionError::Recursion {
                property: self.tree.fully_qualified_name(node),
            }
            .into());
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }

    /// `FGPropertyValue::GetValue()`: the node's value times the sign.
    pub fn property(&mut self, value: &PropertyValue) -> Result<f64, Error> {
        let node = value.node(self.tree)?;
        Ok(self.read_node(node)? * value.sign())
    }

    /// The value of one operand.
    pub fn parameter(&mut self, parameter: &Parameter) -> Result<f64, Error> {
        match parameter {
            Parameter::Real(v) => Ok(*v),
            Parameter::Property(p) => self.property(p),
            Parameter::Function(id) => self.function(*id),
            Parameter::Table(id) => self.table(*id),
            Parameter::Template { template, property } => {
                let node = property.node(self.tree)?;
                let saved = self.template_argument.replace(node);
                let result = self.function(*template);
                self.template_argument = saved;
                result
            }
            Parameter::TemplateArgument(at) => {
                let node = self
                    .template_argument
                    .ok_or_else(|| FunctionError::UnboundTemplateParameter { at: at.clone() })?;
                self.read_node(node)
            }
        }
    }

    /// `GetValue()` of function `id`.
    pub fn function(&mut self, id: FnId) -> Result<f64, Error> {
        let set = self.set;
        let function = set.function(id)?;
        if let Some(cached) = function.cached {
            return Ok(cached);
        }
        let value = self.apply(function)?;
        if let Some(node) = function.copy_to {
            self.tree.set_double(node, value);
        }
        Ok(value)
    }

    /// `GetValue()` of table `id`: looks up the values of its properties.
    /// The keys are read in axis order (row, column, table, ...) before
    /// the table interpolates.
    pub fn table(&mut self, id: TableId) -> Result<f64, Error> {
        let set = self.set;
        let table = set.table(id)?;
        match table.kind() {
            TableKind::OneD => Ok(table.value_1d(self.table_key(table, 0)?)),
            TableKind::TwoD => {
                let row = self.table_key(table, 0)?;
                let column = self.table_key(table, 1)?;
                Ok(table.value_2d(row, column))
            }
            TableKind::Nested => {
                for axis in 0..table.dimensions() {
                    let key = self.table_key(table, axis)?;
                    table.set_key(axis, key);
                }
                Ok(table.value_of_keys())
            }
        }
    }

    /// The value of lookup property `axis` of `table`; NaN without one.
    fn table_key(&mut self, table: &Table, axis: usize) -> Result<f64, Error> {
        match table.lookup_properties().get(axis) {
            Some(Some(p)) => self.property(p),
            _ => Ok(f64::NAN),
        }
    }
}
