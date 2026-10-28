// SPDX-License-Identifier: LGPL-2.1-or-later
// props.cxx: Started Fall 2000 by David Megginson, david@megginson.com.
// This code is released into the Public Domain.
// Derived from JSBSim src/simgear/props/props.cxx (value access).
//
//! Typed node values and SimGear's conversion rules between them.

use tracing::info;

use super::conversion::{Written, parse_into};
use super::{Attribute, NodeId, PropertyTree, ValueType};
use crate::c_numeric::{atoi, format_g, strtod, strtol};

/// Value held by a node.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// No value yet.
    None,
    /// Reads and writes are forwarded to another node.
    Alias(NodeId),
    /// `bool`.
    Bool(bool),
    /// C `int`.
    Int(i32),
    /// C `long`.
    Long(i64),
    /// C `float`.
    Float(f32),
    /// C `double`.
    Double(f64),
    /// Text.
    String(String),
    /// Text whose type is decided by the first typed write.
    Unspecified(String),
}

impl Value {
    fn value_type(&self) -> Option<ValueType> {
        Some(match self {
            Self::None => ValueType::None,
            Self::Alias(_) => return None,
            Self::Bool(_) => ValueType::Bool,
            Self::Int(_) => ValueType::Int,
            Self::Long(_) => ValueType::Long,
            Self::Float(_) => ValueType::Float,
            Self::Double(_) => ValueType::Double,
            Self::String(_) => ValueType::String,
            Self::Unspecified(_) => ValueType::Unspecified,
        })
    }
}

impl PropertyTree {
    /// Follows aliases from `id`, returning the node that holds the value,
    /// or `None` when some node on the way is not readable.
    #[inline]
    pub fn read_target(&self, mut id: NodeId) -> Option<NodeId> {
        loop {
            let node = self.node(id);
            if node.attributes & Attribute::TRACE_READ.0 != 0 {
                self.trace_read(id);
            }
            if node.attributes & Attribute::READ.0 == 0 {
                return None;
            }
            match node.value {
                Value::Alias(target) => id = target,
                _ => return Some(id),
            }
        }
    }

    /// Kept out of line so that [`read_target`](Self::read_target), on
    /// every property read, stays small enough to inline.
    #[cold]
    #[inline(never)]
    fn trace_read(&self, id: NodeId) {
        info!("TRACE: Read node {}", self.path(id, false));
    }

    fn read<T>(&self, id: NodeId, default: T, convert: impl Fn(&Value) -> T) -> T {
        self.read_target(id)
            .map_or(default, |target| convert(&self.node(target).value))
    }

    /// Type of the value, looking through aliases (`getType`).
    #[must_use]
    pub fn value_type(&self, id: NodeId) -> ValueType {
        let mut id = id;
        loop {
            match &self.node(id).value {
                Value::Alias(target) => id = *target,
                other => return other.value_type().unwrap_or(ValueType::None),
            }
        }
    }

    /// Whether `id` aliases another node, and which.
    #[must_use]
    pub fn alias_target(&self, id: NodeId) -> Option<NodeId> {
        match self.node(id).value {
            Value::Alias(target) => Some(target),
            _ => None,
        }
    }

    /// The raw value of `id` itself, without following aliases.
    #[must_use]
    pub fn raw_value(&self, id: NodeId) -> &Value {
        &self.node(id).value
    }

    /// `getBoolValue`.
    #[must_use]
    pub fn get_bool(&self, id: NodeId) -> bool {
        self.read(id, false, |v| match v {
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Long(l) => *l != 0,
            Value::Float(f) => *f != 0.0,
            Value::Double(d) => *d != 0.0,
            Value::String(s) | Value::Unspecified(s) => s == "true" || strtod(s) != 0.0,
            Value::None | Value::Alias(_) => false,
        })
    }

    /// `getIntValue`.
    #[must_use]
    pub fn get_int(&self, id: NodeId) -> i32 {
        self.read(id, 0, |v| match v {
            Value::Bool(b) => i32::from(*b),
            Value::Int(i) => *i,
            Value::Long(l) => *l as i32,
            Value::Float(f) => *f as i32,
            Value::Double(d) => *d as i32,
            Value::String(s) | Value::Unspecified(s) => atoi(s),
            Value::None | Value::Alias(_) => 0,
        })
    }

    /// `getLongValue`.
    #[must_use]
    pub fn get_long(&self, id: NodeId) -> i64 {
        self.read(id, 0, |v| match v {
            Value::Bool(b) => i64::from(*b),
            Value::Int(i) => i64::from(*i),
            Value::Long(l) => *l,
            Value::Float(f) => *f as i64,
            Value::Double(d) => *d as i64,
            Value::String(s) | Value::Unspecified(s) => strtol(s, 0),
            Value::None | Value::Alias(_) => 0,
        })
    }

    /// `getFloatValue`.
    #[must_use]
    pub fn get_float(&self, id: NodeId) -> f32 {
        self.read(id, 0.0, |v| match v {
            Value::Bool(b) => f32::from(u8::from(*b)),
            Value::Int(i) => *i as f32,
            Value::Long(l) => *l as f32,
            Value::Float(f) => *f,
            Value::Double(d) => *d as f32,
            Value::String(s) | Value::Unspecified(s) => strtod(s) as f32,
            Value::None | Value::Alias(_) => 0.0,
        })
    }

    /// `getDoubleValue`.
    #[must_use]
    pub fn get_double(&self, id: NodeId) -> f64 {
        self.read(id, 0.0, |v| match v {
            Value::Bool(b) => f64::from(u8::from(*b)),
            Value::Int(i) => f64::from(*i),
            Value::Long(l) => *l as f64,
            Value::Float(f) => f64::from(*f),
            Value::Double(d) => *d,
            Value::String(s) | Value::Unspecified(s) => strtod(s),
            Value::None | Value::Alias(_) => 0.0,
        })
    }

    /// `getStringValue`: numbers are formatted as SimGear's `make_string`
    /// does (`double` with 10 significant digits, `float` with 6).
    #[must_use]
    pub fn get_string(&self, id: NodeId) -> String {
        self.read(id, String::new(), |v| match v {
            Value::Bool(b) => b.to_text(),
            Value::Int(i) => i.to_string(),
            Value::Long(l) => l.to_string(),
            Value::Float(f) => format_g(f64::from(*f), 6),
            Value::Double(d) => format_g(*d, 10),
            Value::String(s) | Value::Unspecified(s) => s.clone(),
            Value::None | Value::Alias(_) => String::new(),
        })
    }

    /// Follows aliases from `id` for a write, returning the node that
    /// receives it, or `None` when some node on the way is not writable.
    pub fn write_target(&self, mut id: NodeId) -> Option<NodeId> {
        loop {
            if !self.attribute(id, Attribute::WRITE) {
                return None;
            }
            match self.node(id).value {
                Value::Alias(target) => id = target,
                _ => return Some(id),
            }
        }
    }

    fn write<T: Written>(&mut self, id: NodeId, value: T) -> bool {
        match self.write_target(id) {
            Some(target) => {
                self.store_typed(target, value);
                self.trace_write(target);
                true
            }
            None => false,
        }
    }

    fn store_typed<T: Written>(&mut self, target: NodeId, value: T) {
        let Some(node) = self.node_mut(target) else {
            return;
        };
        let slot = &mut node.value;
        *slot = match &*slot {
            Value::None | Value::Unspecified(_) | Value::Alias(_) => value.into_value(),
            Value::Bool(_) => Value::Bool(value.to_bool()),
            Value::Int(_) => Value::Int(value.to_int()),
            Value::Long(_) => Value::Long(value.to_long()),
            Value::Float(_) => Value::Float(value.to_float()),
            Value::Double(_) => Value::Double(value.to_double()),
            Value::String(_) => Value::String(value.to_text()),
        };
    }

    fn trace_write(&self, id: NodeId) {
        if self.attribute(id, Attribute::TRACE_WRITE) {
            info!(
                "TRACE: Write node {}, value \"{}\"",
                self.path(id, false),
                self.get_string(id)
            );
        }
    }

    /// `setBoolValue`; false when the node is not writable.
    pub fn set_bool(&mut self, id: NodeId, value: bool) -> bool {
        self.write(id, value)
    }

    /// `setIntValue`; false when the node is not writable.
    pub fn set_int(&mut self, id: NodeId, value: i32) -> bool {
        self.write(id, value)
    }

    /// `setLongValue`; false when the node is not writable.
    pub fn set_long(&mut self, id: NodeId, value: i64) -> bool {
        self.write(id, value)
    }

    /// `setFloatValue`; false when the node is not writable.
    pub fn set_float(&mut self, id: NodeId, value: f32) -> bool {
        self.write(id, value)
    }

    /// `setDoubleValue`; false when the node is not writable.
    pub fn set_double(&mut self, id: NodeId, value: f64) -> bool {
        self.write(id, value)
    }

    /// `setStringValue`: an untyped node becomes a string; a typed node
    /// parses the text.
    pub fn set_string(&mut self, id: NodeId, text: &str) -> bool {
        self.write_text(id, text, false)
    }

    /// `setUnspecifiedValue`: like [`set_string`](Self::set_string), but an
    /// empty node keeps its type open.
    pub fn set_unspecified(&mut self, id: NodeId, text: &str) -> bool {
        self.write_text(id, text, true)
    }

    fn write_text(&mut self, id: NodeId, text: &str, unspecified: bool) -> bool {
        let Some(target) = self.write_target(id) else {
            return false;
        };
        let Some(node) = self.node_mut(target) else {
            return false;
        };
        let slot = &mut node.value;
        *slot = match &*slot {
            Value::None if unspecified => Value::Unspecified(text.to_owned()),
            Value::None => Value::String(text.to_owned()),
            Value::Unspecified(_) if !unspecified => Value::String(text.to_owned()),
            other => parse_into(other, text),
        };
        self.trace_write(target);
        true
    }

    /// Makes `id` forward to `target` (`alias`). Fails when `id` is tied,
    /// already aliases another node, or the alias would form a cycle; the
    /// last case recurses without end in SimGear.
    pub fn alias(&mut self, id: NodeId, target: NodeId) -> bool {
        if self.alias_target(id) == Some(target) {
            return true;
        }
        if self.alias_target(id).is_some() || self.is_tied(id) {
            return false;
        }
        let mut probe = Some(target);
        while let Some(node) = probe {
            if node == id {
                return false;
            }
            probe = self.alias_target(node);
        }
        let Some(node) = self.node_mut(id) else {
            return false;
        };
        node.value = Value::Alias(target);
        true
    }

    /// Removes an alias (`unalias`); false when `id` is not an alias.
    pub fn unalias(&mut self, id: NodeId) -> bool {
        if self.alias_target(id).is_none() {
            return false;
        }
        let Some(node) = self.node_mut(id) else {
            return false;
        };
        node.value = Value::None;
        true
    }

    /// Ties `id` to a model, which from now on owns its value; the node
    /// starts with `initial` (`FGPropertyManager::Tie`). Without a setter
    /// (`writable == false`) the node loses [`Attribute::WRITE`]. Fails on
    /// aliases and already tied nodes.
    pub fn tie(&mut self, id: NodeId, initial: Value, writable: bool) -> bool {
        if self.alias_target(id).is_some() || self.is_tied(id) {
            return false;
        }
        let saved = (
            self.attribute(id, Attribute::READ),
            self.attribute(id, Attribute::WRITE),
        );
        let Some(node) = self.node_mut(id) else {
            return false;
        };
        node.value = match initial {
            Value::Alias(_) | Value::Unspecified(_) => Value::None,
            other => other,
        };
        node.tied = Some(saved);
        if !writable {
            self.set_attribute(id, Attribute::WRITE, false);
        }
        true
    }

    /// Releases a tie, restoring READ and WRITE as they were before it
    /// (`FGPropertyManager::Untie`). The node keeps its current value.
    pub fn untie(&mut self, id: NodeId) -> bool {
        let Some((read, write)) = self.node(id).tied else {
            return false;
        };
        self.set_attribute(id, Attribute::READ, read);
        self.set_attribute(id, Attribute::WRITE, write);
        let Some(node) = self.node_mut(id) else {
            return false;
        };
        node.tied = None;
        // SimGear re-reads the value through the getter, which yields the
        // type's default when READ is off.
        node.value = match &node.value {
            Value::Unspecified(text) | Value::String(text) if read => Value::String(text.clone()),
            other if read => other.clone(),
            Value::Bool(_) => Value::Bool(false),
            Value::Int(_) => Value::Int(0),
            Value::Long(_) => Value::Long(0),
            Value::Float(_) => Value::Float(0.0),
            Value::Double(_) => Value::Double(0.0),
            Value::String(_) | Value::Unspecified(_) => Value::String(String::new()),
            other => other.clone(),
        };
        true
    }

    /// Stores into a tied node on behalf of its owner, bypassing the WRITE
    /// attribute that guards it from outside writers.
    pub fn store(&mut self, id: NodeId, value: f64) {
        self.store_typed(id, value);
    }
}
