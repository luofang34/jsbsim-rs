// SPDX-License-Identifier: LGPL-2.1-or-later
// FGParameter, FGRealValue, FGPropertyValue: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// FGParameterValue, FGFunctionValue: Copyright (C) 2018 B. Coconnier (bcoconni@users.sf.net).
// Derived from JSBSim src/math/FGParameter.h, FGRealValue.{h,cpp},
// FGPropertyValue.{h,cpp}, FGParameterValue.h and FGFunctionValue.h.
//
//! Operands of functions, tables and conditions.

use std::cell::Cell;

use super::{FnId, FunctionError, TableId};
use crate::Error;
use crate::properties::{Attribute, NodeId, PropertyTree};
use crate::xml::{Location, atof_locale_c};

/// A property operand, bound to its node now or at first read
/// (`FGPropertyValue`). A leading `-` negates the value.
#[derive(Debug, Clone)]
pub struct PropertyValue {
    name: String,
    sign: f64,
    /// Bound at first read when the property did not exist at load, as
    /// `FGPropertyValue::GetNode` does: a property created after load
    /// still binds, and a missing one fails at that first evaluation, not
    /// at load. The evaluator holds operands through a shared borrow of the
    /// function set, hence `Cell`, which is `Send`.
    node: Cell<Option<NodeId>>,
    at: Location,
}

impl PropertyValue {
    /// Binds to `name` if it already exists; otherwise binds at first read.
    pub fn new(name: &str, tree: &PropertyTree, at: Location) -> Result<Self, Error> {
        let (name, sign) = match name.strip_prefix('-') {
            Some(rest) => (rest, -1.0),
            None => (name, 1.0),
        };
        let node = if tree.has_node(name)? {
            tree.get(name)?
        } else {
            None
        };
        Ok(Self {
            name: name.to_owned(),
            sign,
            node: Cell::new(node),
            at,
        })
    }

    /// Property path without sign.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// -1 for a negated operand, 1 otherwise.
    #[must_use]
    pub fn sign(&self) -> f64 {
        self.sign
    }

    /// Whether the node is still unknown (`IsLateBound`).
    #[must_use]
    pub fn is_late_bound(&self) -> bool {
        self.node.get().is_none()
    }

    /// The node, binding it now if needed (`GetNode`).
    pub fn node(&self, tree: &PropertyTree) -> Result<NodeId, Error> {
        if let Some(node) = self.node.get() {
            return Ok(node);
        }
        let node = tree
            .get(&self.name)?
            .ok_or_else(|| FunctionError::MissingProperty {
                property: self.name.clone(),
                at: self.at.clone(),
            })?;
        self.node.set(Some(node));
        Ok(node)
    }

    /// `IsConstant`: bound to a node that is neither tied nor writable.
    #[must_use]
    pub fn is_constant(&self, tree: &PropertyTree) -> bool {
        self.node
            .get()
            .is_some_and(|n| !tree.is_tied(n) && !tree.attribute(n, Attribute::WRITE))
    }
}

/// One operand.
#[derive(Debug, Clone)]
pub enum Parameter {
    /// A constant (`FGRealValue`).
    Real(f64),
    /// A property (`FGPropertyValue`).
    Property(PropertyValue),
    /// A nested function.
    Function(FnId),
    /// A table.
    Table(TableId),
    /// A template applied to a property (`FGFunctionValue`).
    Template {
        /// The template function.
        template: FnId,
        /// The property it is applied to.
        property: PropertyValue,
    },
    /// The `#` placeholder inside a template body.
    TemplateArgument(Location),
}

impl Parameter {
    /// `FGParameterValue(value)`: a number if `text` is one, else a
    /// property.
    pub fn number_or_property(
        text: &str,
        tree: &PropertyTree,
        at: Location,
    ) -> Result<Self, Error> {
        match atof_locale_c(text) {
            Ok(value) => Ok(Self::Real(value)),
            Err(_) => Ok(Self::Property(PropertyValue::new(text, tree, at)?)),
        }
    }
}
