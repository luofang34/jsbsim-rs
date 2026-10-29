// SPDX-License-Identifier: LGPL-2.1-or-later
// FGFunction: Copyright (C) 2004 Jon S. Berndt (jon@jsbsim.org).
// FGTable, FGPropertyValue: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/math/FGFunction.cpp, FGTable.cpp and
// FGPropertyValue.cpp (GetValue) and src/simgear/props/props.cxx
// (getDoubleValue of tied nodes).
//
//! Evaluation of parameters, functions and tables against a property tree.
//!
//! Inside the evaluator an operand's value is an [`Eval`], whose error
//! carries nothing: the error itself waits in the evaluator until the
//! public method that started the evaluation returns it. A `Result<f64,
//! Error>` is returned through memory and an `Eval` in two registers, on
//! every operand of every function.

mod apply;

use super::{
    Bound, FnId, Function, FunctionError, FunctionSet, Parameter, PropertyValue, Table, TableId,
    TableKind,
};
use crate::Error;
use crate::properties::{NodeId, PropertyTree};

/// Deeper nesting than this means a property reads itself.
const MAX_DEPTH: u32 = 256;

/// An evaluation failed; the evaluator holds the error.
#[derive(Debug)]
struct Failed;

/// An operand's value inside the evaluator.
type Eval = Result<f64, Failed>;

/// Values of nodes tied to model getters, which JSBSim evaluates on every
/// read of the node.
pub trait GetterSource {
    /// The getter's value if `node` is tied to one.
    fn get(&self, node: NodeId) -> Option<f64>;
}

/// A source with no getters, for function sets used on their own.
pub struct NoGetters;

impl GetterSource for NoGetters {
    #[inline]
    fn get(&self, _node: NodeId) -> Option<f64> {
        None
    }
}

/// Reads properties and evaluates functions and tables, with the tied
/// properties' getters from `G`. The source is a type parameter rather
/// than a trait object, so that a getter read on the frame's hot path is
/// a direct, inlinable call.
pub struct Evaluator<'a, G = NoGetters> {
    set: &'a FunctionSet,
    tree: &'a mut PropertyTree,
    getters: G,
    depth: u32,
    template_argument: Option<NodeId>,
    /// The error of the evaluation that returned [`Failed`].
    failure: Option<Error>,
}

impl<'a> Evaluator<'a> {
    /// An evaluator over `set` and `tree`, without model getters.
    pub fn new(set: &'a FunctionSet, tree: &'a mut PropertyTree) -> Self {
        Self::with_getters(set, tree, NoGetters)
    }
}

impl<'a, G: GetterSource> Evaluator<'a, G> {
    /// An evaluator that also reads model getters from `getters`.
    pub fn with_getters(set: &'a FunctionSet, tree: &'a mut PropertyTree, getters: G) -> Self {
        Self {
            set,
            tree,
            getters,
            depth: 0,
            template_argument: None,
            failure: None,
        }
    }

    /// The tree being read.
    pub fn tree(&mut self) -> &mut PropertyTree {
        self.tree
    }

    /// `getDoubleValue()` of `node`: aliases are followed, an unreadable
    /// node reads 0, and a node bound to a function or table evaluates it
    /// and stores the result.
    #[inline]
    pub fn read_node(&mut self, node: NodeId) -> Result<f64, Error> {
        let value = self.eval_node(node);
        self.finish(value)
    }

    /// `FGPropertyValue::GetValue()`: the node's value times the sign.
    #[inline]
    pub fn property(&mut self, value: &PropertyValue) -> Result<f64, Error> {
        let value = self.eval_property(value);
        self.finish(value)
    }

    /// The value of one operand.
    #[inline]
    pub fn parameter(&mut self, parameter: &Parameter) -> Result<f64, Error> {
        let value = self.eval_parameter(parameter);
        self.finish(value)
    }

    /// `GetValue()` of function `id`.
    #[inline]
    pub fn function(&mut self, id: FnId) -> Result<f64, Error> {
        let value = self.eval_function(id);
        self.finish(value)
    }

    /// `GetValue()` of table `id`: looks up the values of its properties.
    /// The keys are read in axis order (row, column, table, ...) before
    /// the table interpolates.
    #[inline]
    pub fn table(&mut self, id: TableId) -> Result<f64, Error> {
        let value = self.eval_table(id);
        self.finish(value)
    }

    /// `value` with the error that made it fail.
    #[inline]
    fn finish(&mut self, value: Eval) -> Result<f64, Error> {
        value.map_err(|Failed| self.take_failure())
    }

    #[cold]
    #[inline(never)]
    fn take_failure(&mut self) -> Error {
        // Only `fail` makes a `Failed`, so the error is there.
        self.failure.take().unwrap_or_else(|| Error::Model {
            message: "a function evaluation failed without recording why".to_owned(),
        })
    }

    /// Records `error` as the evaluation's.
    #[cold]
    #[inline(never)]
    fn fail(&mut self, error: impl Into<Error>) -> Failed {
        self.failure = Some(error.into());
        Failed
    }

    fn eval_node(&mut self, node: NodeId) -> Eval {
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
            Bound::Function(id) => e.eval_function(id),
            Bound::Table(id) => e.eval_table(id),
        })?;
        self.tree.store(target, value);
        Ok(value)
    }

    fn nested(&mut self, node: NodeId, f: impl FnOnce(&mut Self) -> Eval) -> Eval {
        if self.depth >= MAX_DEPTH {
            let property = self.tree.fully_qualified_name(node);
            return Err(self.fail(FunctionError::Recursion { property }));
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }

    /// The node of `value`, binding it now if needed.
    #[inline]
    fn value_node(&mut self, value: &PropertyValue) -> Result<NodeId, Failed> {
        value.node(self.tree).map_err(|e| self.fail(e))
    }

    #[inline]
    fn eval_property(&mut self, value: &PropertyValue) -> Eval {
        let node = self.value_node(value)?;
        Ok(self.eval_node(node)? * value.sign())
    }

    fn eval_parameter(&mut self, parameter: &Parameter) -> Eval {
        match parameter {
            Parameter::Real(v) => Ok(*v),
            Parameter::Property(p) => self.eval_property(p),
            Parameter::Function(id) => self.eval_function(*id),
            Parameter::Table(id) => self.eval_table(*id),
            Parameter::Template { template, property } => {
                let node = self.value_node(property)?;
                let saved = self.template_argument.replace(node);
                let result = self.eval_function(*template);
                self.template_argument = saved;
                result
            }
            Parameter::TemplateArgument(at) => {
                let Some(node) = self.template_argument else {
                    let at = at.clone();
                    return Err(self.fail(FunctionError::UnboundTemplateParameter { at }));
                };
                self.eval_node(node)
            }
        }
    }

    #[inline]
    fn eval_function(&mut self, id: FnId) -> Eval {
        let set = self.set;
        let function = set.function(id).map_err(|e| self.fail(e))?;
        if let Some(cached) = function.cached {
            return Ok(cached);
        }
        self.evaluate(function)
    }

    /// `GetValue()` of an uncached function. Out of line, so that a cached
    /// read does not pay for the registers and stack its operations take.
    #[inline(never)]
    fn evaluate(&mut self, function: &Function) -> Eval {
        let value = self.apply(function)?;
        if let Some(node) = function.copy_to {
            self.tree.set_double(node, value);
        }
        Ok(value)
    }

    fn eval_table(&mut self, id: TableId) -> Eval {
        let set = self.set;
        let table = set.table(id).map_err(|e| self.fail(e))?;
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
    fn table_key(&mut self, table: &Table, axis: usize) -> Eval {
        match table.lookup_properties().get(axis) {
            Some(Some(p)) => self.eval_property(p),
            _ => Ok(f64::NAN),
        }
    }
}
