// SPDX-License-Identifier: LGPL-2.1-or-later
// FGPropertyManager: Copyright (C) 2002 Tony Peden.
// Derived from JSBSim src/input_output/FGPropertyManager.h (Tie).
//
//! Properties tied to model getters and setters.
//!
//! JSBSim ties a property to C++ getter and setter methods; every read calls
//! the getter and every write the setter. Here a tie registers closures over
//! [`Models`]: reads go through [`Evaluator`](crate::functions::Evaluator),
//! which asks [`ModelGetters`] for the getter's value, and writes go through
//! [`Fdm::set_property`](super::Fdm::set_property), which calls the setter.

use super::Models;
use crate::Error;
use crate::functions::GetterSource;
use crate::properties::{NodeId, NodeMap, PropertyTree, Value};

/// A getter: the property's value given the models.
pub type Getter = Box<dyn Fn(&Models) -> f64 + Send>;
/// A setter: applies a written value to the models.
pub type Setter = Box<dyn Fn(&mut Models, f64) + Send>;

/// C type of a tied value, which decides how writes and string reads
/// convert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `double`.
    Double,
    /// `bool`: writes store `value != 0`.
    Bool,
    /// `int`: writes truncate.
    Int,
}

/// Getters and setters of tied properties.
#[derive(Default)]
pub struct Bindings {
    getters: NodeMap<Getter>,
    setters: NodeMap<(Kind, Setter)>,
}

impl Bindings {
    /// The setter of `node`, if it has one.
    pub(crate) fn setter(&self, node: NodeId) -> Option<&(Kind, Setter)> {
        self.setters.get(node)
    }

    /// Whether `node` has a getter.
    #[must_use]
    pub fn is_bound(&self, node: NodeId) -> bool {
        self.getters.contains(node)
    }
}

/// Registers ties while models are constructed (`bind()` in JSBSim).
pub struct Binder<'a> {
    pub(crate) tree: &'a mut PropertyTree,
    pub(crate) bindings: &'a mut Bindings,
}

impl Binder<'_> {
    fn tie(
        &mut self,
        path: &str,
        kind: Kind,
        getter: Getter,
        setter: Option<Setter>,
    ) -> Result<NodeId, Error> {
        let node = self.tree.get_or_create(self.tree.root(), path)?;
        let initial = match kind {
            Kind::Double => Value::Double(0.0),
            Kind::Bool => Value::Bool(false),
            Kind::Int => Value::Int(0),
        };
        if !self.tree.tie(node, initial, setter.is_some()) {
            tracing::error!("Failed to tie property {path} to object methods");
            return Ok(node);
        }
        self.bindings.getters.insert(node, getter);
        if let Some(setter) = setter {
            self.bindings.setters.insert(node, (kind, setter));
        }
        Ok(node)
    }

    /// A read-only `double` property.
    pub fn getter(
        &mut self,
        path: &str,
        get: impl Fn(&Models) -> f64 + Send + 'static,
    ) -> Result<NodeId, Error> {
        self.tie(path, Kind::Double, Box::new(get), None)
    }

    /// A read-write property of type `kind`.
    pub fn property(
        &mut self,
        path: &str,
        kind: Kind,
        get: impl Fn(&Models) -> f64 + Send + 'static,
        set: impl Fn(&mut Models, f64) + Send + 'static,
    ) -> Result<NodeId, Error> {
        self.tie(path, kind, Box::new(get), Some(Box::new(set)))
    }

    /// A write-only property (a tie without getter, which clears READ).
    pub fn setter_only(
        &mut self,
        path: &str,
        kind: Kind,
        set: impl Fn(&mut Models, f64) + Send + 'static,
    ) -> Result<NodeId, Error> {
        let node = self.tie(path, kind, Box::new(|_| 0.0), Some(Box::new(set)))?;
        self.tree
            .set_attribute(node, crate::properties::Attribute::READ, false);
        Ok(node)
    }

    /// A read-only property of type `kind` (e.g. an `int` counter).
    pub fn typed_getter(
        &mut self,
        path: &str,
        kind: Kind,
        get: impl Fn(&Models) -> f64 + Send + 'static,
    ) -> Result<NodeId, Error> {
        self.tie(path, kind, Box::new(get), None)
    }
}

/// [`GetterSource`] over the models and their bindings.
pub struct ModelGetters<'a> {
    pub(crate) bindings: &'a Bindings,
    pub(crate) models: &'a Models,
}

impl GetterSource for ModelGetters<'_> {
    fn get(&self, node: NodeId) -> Option<f64> {
        self.bindings.getters.get(node).map(|g| g(self.models))
    }
}

/// How a written `f64` lands in a tied value of `kind`, as SimGear's
/// `setDoubleValue` converts it.
#[must_use]
pub fn convert_write(kind: Kind, value: f64) -> f64 {
    match kind {
        Kind::Double => value,
        Kind::Bool => f64::from(u8::from(value != 0.0)),
        Kind::Int => f64::from(value as i32),
    }
}
