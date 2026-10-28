// SPDX-License-Identifier: LGPL-2.1-or-later
// FGFunction: Copyright (C) 2004 Jon S. Berndt (jon@jsbsim.org).
// FGTable, FGPropertyValue: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/math/FGFunction.cpp (Load, CreateOutputNode,
// argument checks, constant folding), FGTable.cpp (bind) and
// FGTemplateFunc.cpp.
//
//! Building functions and tables from XML into a [`FunctionSet`].

use std::f64::consts::PI;

use super::ops::{Parity, UNSUPPORTED};
use super::{
    Bound, Evaluator, FnId, Function, FunctionError, FunctionSet, Op, Parameter, PropertyValue,
    Table, TableId, is_number, output_name,
};
use crate::Error;
use crate::properties::{PropertyTree, mk_property_name};
use crate::xml::Element;

/// Whether `#` operands stand for a template argument.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Body {
    Plain,
    Template,
}

impl FunctionSet {
    /// `FGFunction(fdmex, el, prefix)`: a `<function>` element with its
    /// single operand, bound to its `name` and copied to `copyto`.
    pub fn load_function(
        &mut self,
        tree: &mut PropertyTree,
        el: &Element,
        prefix: &str,
    ) -> Result<FnId, Error> {
        let id = self.load_operation(tree, el, Op::Identity, prefix, Body::Plain)?;
        let copy_to = el.attribute("copyto");
        if copy_to.is_empty() {
            return Ok(id);
        }
        let copy_to = if !copy_to.contains('#') {
            copy_to.to_owned()
        } else if is_number(prefix) {
            copy_to.replacen('#', prefix, 1)
        } else {
            tracing::error!(
                "{}: Illegal use of the special character '#'. The 'copyto' argument in function {} is ignored.",
                el.location(),
                self.function(id).name
            );
            return Ok(id);
        };
        match tree.get(&copy_to)? {
            Some(node) => self.functions[id.0].copy_to = Some(node),
            None => tracing::error!(
                "{}: Property \"{copy_to}\" must be previously defined in function {}. The 'copyto' argument is ignored.",
                el.location(),
                self.function(id).name
            ),
        }
        Ok(id)
    }

    /// `AddTemplateFunc(name, el)`: a function whose `#` operands stand for
    /// the property it is applied to.
    pub fn add_template(
        &mut self,
        tree: &mut PropertyTree,
        name: &str,
        el: &Element,
    ) -> Result<FnId, Error> {
        let id = self.load_operation(tree, el, Op::Identity, "", Body::Template)?;
        self.templates.insert(name.to_owned(), id);
        Ok(id)
    }

    /// `FGTable(pm, el, prefix)`, bound to its `name` unless internal.
    pub fn load_table(
        &mut self,
        tree: &mut PropertyTree,
        el: &Element,
        prefix: &str,
    ) -> Result<TableId, Error> {
        let table = Table::load(el, tree, prefix)?;
        let name = table.name().to_owned();
        let internal = table.is_internal();
        let id = self.push_table(table);
        if name.is_empty() || internal {
            return Ok(id);
        }
        let name = if prefix.is_empty() {
            name
        } else if !is_number(prefix) {
            format!("{prefix}/{name}")
        } else if name.contains('#') {
            name.replacen('#', prefix, 1)
        } else {
            return Err(FunctionError::Table {
                at: el.location(),
                message: format!(
                    "Malformed table name with number: {prefix} and property name: {name} but no \"#\" sign for substitution."
                ),
            }
            .into());
        };
        let property = mk_property_name(&name, false);
        if tree.has_node(&property)? && tree.get(&property)?.is_some_and(|n| tree.is_tied(n)) {
            return Err(FunctionError::AlreadyBound {
                property,
                at: el.location(),
            }
            .into());
        }
        self.bind(tree, &property, Bound::Table(id), &el.location())?;
        Ok(id)
    }

    /// Loads `el` as operation `op`: its operands, then its output binding,
    /// then the argument checks, in JSBSim's order.
    fn load_operation(
        &mut self,
        tree: &mut PropertyTree,
        el: &Element,
        op: Op,
        prefix: &str,
        body: Body,
    ) -> Result<FnId, Error> {
        let params = self.load_operands(tree, el, prefix, body)?;
        let mut name = el.attribute("name").to_owned();
        let at = el.location();
        let output = if body == Body::Template && op == Op::Identity {
            None
        } else {
            output_name(&name, prefix, &at)
        };
        if is_number(prefix) {
            name = name.replacen('#', prefix, 1);
        }
        let id = self.push_function(Function {
            name,
            op,
            params,
            node: None,
            copy_to: None,
            cached: None,
            at,
        });
        if let Some(property) = output {
            let node = self.bind(tree, &property, Bound::Function(id), &el.location())?;
            self.functions[id.0].node = Some(node);
        }
        self.check_arity(id, el)?;
        Ok(id)
    }

    fn check_arity(&self, id: FnId, el: &Element) -> Result<(), Error> {
        let function = self.function(id);
        let (min, max, parity) = function.op.arity();
        let n = function.params.len();
        let message = if n < min {
            format!("should have at least {min} argument(s).")
        } else if n > max {
            format!("should have no more than {max} argument(s).")
        } else if parity == Parity::Odd && n.is_multiple_of(2) {
            "must have an odd number of arguments.".to_owned()
        } else {
            return Ok(());
        };
        Err(FunctionError::Arguments {
            element: el.name().to_owned(),
            at: el.location(),
            message,
        }
        .into())
    }

    fn load_operands(
        &mut self,
        tree: &mut PropertyTree,
        el: &Element,
        prefix: &str,
        body: Body,
    ) -> Result<Vec<Parameter>, Error> {
        let mut params = Vec::new();
        for child in el.children() {
            if let Some(param) = self.load_operand(tree, el, child, prefix, body)? {
                params.push(param);
            }
            self.fold_last(tree, &mut params)?;
        }
        Ok(params)
    }

    fn load_operand(
        &mut self,
        tree: &mut PropertyTree,
        parent: &Element,
        el: &Element,
        prefix: &str,
        body: Body,
    ) -> Result<Option<Parameter>, Error> {
        let name = el.name();
        Ok(Some(match name {
            "property" | "p" => return self.load_property(tree, el, prefix, body),
            "value" | "v" => Parameter::Real(el.data_as_number()?),
            "pi" => Parameter::Real(PI),
            "table" | "t" => {
                if el.attribute("type") == "internal" {
                    return Err(FunctionError::InternalTableInFunction {
                        at: parent.location(),
                    }
                    .into());
                }
                Parameter::Table(self.load_table(tree, el, prefix)?)
            }
            "description" => return Ok(None),
            _ => match Op::from_name(name) {
                Some(op) if UNSUPPORTED.contains(&op) => {
                    return Err(Error::Unsupported {
                        construct: format!("<{name}>"),
                        path: el.location().to_string(),
                    });
                }
                Some(op) => return self.load_nested(tree, el, op, prefix, body).map(Some),
                None => {
                    tracing::error!(
                        "{}: Bad operation <{name}> detected in configuration file",
                        el.location()
                    );
                    return Ok(None);
                }
            },
        }))
    }

    /// A nested operation; variadic ones with a single operand collapse to
    /// that operand, as `VarArgsFn` does.
    fn load_nested(
        &mut self,
        tree: &mut PropertyTree,
        el: &Element,
        op: Op,
        prefix: &str,
        body: Body,
    ) -> Result<Parameter, Error> {
        let error = match self.load_operation(tree, el, op, prefix, body) {
            Ok(id) => return Ok(Parameter::Function(id)),
            Err(e) => e,
        };
        let own_count_error = matches!(
            &error,
            Error::Function(FunctionError::Arguments { at, .. }) if *at == el.location()
        );
        let collapsible = self
            .functions
            .last()
            .filter(|f| f.op == op && f.params.len() == 1);
        let (Some(function), true, true) =
            (collapsible, own_count_error, op.collapses_single_argument())
        else {
            return Err(error);
        };
        tracing::warn!(
            "{}: <{}> only has one argument which makes it a no-op. Its argument will be evaluated but <{}> will not be applied to the result.",
            el.location(),
            op.name(),
            op.name()
        );
        let (node, param) = (function.node, function.params[0].clone());
        if let Some(node) = node {
            // JSBSim's destructor unties the node, which keeps the value its
            // getter returns at that moment.
            self.bound.remove(&node);
            tree.untie(node);
            if let Ok(value) = Evaluator::new(self, tree).parameter(&param) {
                tree.store(node, value);
            }
        }
        Ok(param)
    }

    fn load_property(
        &mut self,
        tree: &mut PropertyTree,
        el: &Element,
        prefix: &str,
        body: Body,
    ) -> Result<Option<Parameter>, Error> {
        let text = el.data_line(0);
        if body == Body::Template && text.trim() == "#" {
            return Ok(Some(Parameter::TemplateArgument(el.location())));
        }
        let name = if !text.contains('#') {
            text.to_owned()
        } else if is_number(prefix) {
            text.replacen('#', prefix, 1)
        } else {
            return Err(FunctionError::IllegalHash { at: el.location() }.into());
        };
        let property = PropertyValue::new(&name, tree, el.location())?;
        if !el.has_attribute("apply") {
            return Ok(Some(Parameter::Property(property)));
        }
        let template_name = el.attribute("apply");
        match self.template(template_name) {
            Some(template) => Ok(Some(Parameter::Template { template, property })),
            None => {
                tracing::error!(
                    "{}: No function by the name {template_name} has been defined. This property will not be logged.",
                    el.location()
                );
                Ok(None)
            }
        }
    }

    /// Replaces the last operand by its value when it is a function of
    /// constants; its property, if any, becomes a read-only plain value.
    fn fold_last(
        &mut self,
        tree: &mut PropertyTree,
        params: &mut [Parameter],
    ) -> Result<(), Error> {
        let Some(Parameter::Function(id)) = params.last().cloned() else {
            return Ok(());
        };
        if !self.is_constant_function(id, tree) {
            return Ok(());
        }
        let value = Evaluator::new(self, tree).function(id)?;
        if let Some(node) = self.function(id).node {
            self.unbind_as_constant(tree, node, value);
        }
        if let Some(last) = params.last_mut() {
            *last = Parameter::Real(value);
        }
        Ok(())
    }

    fn is_constant_function(&self, id: FnId, tree: &PropertyTree) -> bool {
        let function = self.function(id);
        !UNSUPPORTED.contains(&function.op)
            && function.params.iter().all(|p| match p {
                Parameter::Real(_) => true,
                Parameter::Property(v) | Parameter::Template { property: v, .. } => {
                    v.is_constant(tree)
                }
                Parameter::Function(inner) => self.is_constant_function(*inner, tree),
                Parameter::Table(_) | Parameter::TemplateArgument(_) => false,
            })
    }
}
