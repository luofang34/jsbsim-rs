// SPDX-License-Identifier: LGPL-2.1-or-later
// props.cxx: Started Fall 2000 by David Megginson, david@megginson.com.
// This code is released into the Public Domain.
// FGPropertyManager: Copyright (C) 2002 Tony Peden.
// Derived from JSBSim src/simgear/props/props.cxx and
// src/input_output/FGPropertyManager.{h,cpp}.
//
//! Property tree with JSBSim's paths and semantics, including aliases and
//! read-only nodes. It is the external API of the simulation.
//!
//! SimGear ties nodes to C++ getters and setters. Here the tree owns every
//! value: a model that publishes state holds the [`NodeId`] of a tied node
//! and stores into it with [`PropertyTree::store`], which ignores the WRITE
//! attribute that keeps outside writers off read-only nodes.

mod conversion;
mod naming;
mod path;
mod value;

#[cfg(test)]
mod tests;

pub use naming::{mk_property_name, printable_name};
pub use path::PathError;
pub use value::Value;

use path::{Component, parse_path};

/// Handle to a node of a [`PropertyTree`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u32);

/// SimGear node attribute bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Attribute(u8);

impl Attribute {
    /// Value may be read.
    pub const READ: Self = Self(1);
    /// Value may be written.
    pub const WRITE: Self = Self(2);
    /// Saved by archives.
    pub const ARCHIVE: Self = Self(4);
    /// Node was removed.
    pub const REMOVED: Self = Self(8);
    /// Reads are logged.
    pub const TRACE_READ: Self = Self(16);
    /// Writes are logged.
    pub const TRACE_WRITE: Self = Self(32);
    /// Saved by user archives.
    pub const USERARCHIVE: Self = Self(64);
    /// Preserved across resets.
    pub const PRESERVE: Self = Self(128);

    /// The raw bit.
    #[must_use]
    pub fn bits(self) -> u8 {
        self.0
    }
}

/// Type of the value a node holds (`simgear::props::Type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueType {
    /// No value yet.
    None,
    /// `bool`.
    Bool,
    /// C `int`.
    Int,
    /// C `long` (64-bit).
    Long,
    /// C `float`.
    Float,
    /// C `double`.
    Double,
    /// Text.
    String,
    /// Text whose type is decided by the first typed write.
    Unspecified,
}

#[derive(Debug, Clone)]
struct Node {
    name: String,
    index: i32,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    value: Value,
    attributes: u8,
    /// READ and WRITE as they were when the node was tied; `Some` while
    /// tied.
    tied: Option<(bool, bool)>,
}

/// Arena of property nodes rooted at an unnamed node.
#[derive(Debug, Clone)]
pub struct PropertyTree {
    nodes: Vec<Node>,
}

impl Default for PropertyTree {
    fn default() -> Self {
        Self::new()
    }
}

impl PropertyTree {
    /// Creates a tree holding only the root.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: vec![Node::new(String::new(), 0, None)],
        }
    }

    /// The root node.
    #[must_use]
    pub fn root(&self) -> NodeId {
        NodeId(0)
    }

    fn node(&self, id: NodeId) -> &Node {
        // NodeIds are only minted by this tree, which never removes nodes.
        &self.nodes[id.0 as usize]
    }

    fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id.0 as usize]
    }

    /// Finds the node at `path` relative to `from` (`getNode(path)`).
    pub fn find(&self, from: NodeId, path: &str) -> Result<Option<NodeId>, PathError> {
        self.find_indexed(from, path, None)
    }

    /// Like [`find`](Self::find) with the index of the last component
    /// replaced by `index` (`getNode(path, index)`).
    pub fn find_indexed(
        &self,
        from: NodeId,
        path: &str,
        index: Option<i32>,
    ) -> Result<Option<NodeId>, PathError> {
        let components = components_with_index(path, index)?;
        let mut current = from;
        for component in &components {
            current = match component {
                Component::Root => self.root(),
                Component::Current => current,
                Component::Parent => self.parent(current).ok_or_else(past_root)?,
                Component::Child(name, index) => match self.child(current, name, *index) {
                    Some(child) => child,
                    None => return Ok(None),
                },
            };
        }
        Ok(self.live(current))
    }

    /// Finds or creates the node at `path` relative to `from`
    /// (`getNode(path, true)`).
    pub fn get_or_create(&mut self, from: NodeId, path: &str) -> Result<NodeId, PathError> {
        self.get_or_create_indexed(from, path, None)
    }

    /// Like [`get_or_create`](Self::get_or_create) with the index of the
    /// last component replaced by `index`.
    pub fn get_or_create_indexed(
        &mut self,
        from: NodeId,
        path: &str,
        index: Option<i32>,
    ) -> Result<NodeId, PathError> {
        let components = components_with_index(path, index)?;
        let mut current = from;
        for component in components {
            current = match component {
                Component::Root => self.root(),
                Component::Current => current,
                Component::Parent => self.parent(current).ok_or_else(past_root)?,
                Component::Child(name, index) => match self.child(current, &name, index) {
                    Some(child) => child,
                    None => self.add_child(current, name, index),
                },
            };
        }
        Ok(current)
    }

    /// Finds `path` from the root (`FGPropertyManager::GetNode`).
    pub fn get(&self, path: &str) -> Result<Option<NodeId>, PathError> {
        self.find(self.root(), path)
    }

    /// Whether `path` exists, ignoring a leading `-` as JSBSim does for
    /// negated inputs (`FGPropertyManager::HasNode`).
    pub fn has_node(&self, path: &str) -> Result<bool, PathError> {
        let path = path.strip_prefix('-').unwrap_or(path);
        Ok(self.get(path)?.is_some())
    }

    fn live(&self, id: NodeId) -> Option<NodeId> {
        (self.node(id).attributes & Attribute::REMOVED.0 == 0).then_some(id)
    }

    fn add_child(&mut self, parent: NodeId, name: String, index: i32) -> NodeId {
        let id = NodeId(u32::try_from(self.nodes.len()).unwrap_or(u32::MAX));
        self.nodes.push(Node::new(name, index, Some(parent)));
        self.node_mut(parent).children.push(id);
        id
    }

    /// Child of `parent` named `name` with `index` (`getChild`).
    #[must_use]
    pub fn child(&self, parent: NodeId, name: &str, index: i32) -> Option<NodeId> {
        self.node(parent).children.iter().copied().find(|&c| {
            let node = self.node(c);
            node.index == index && node.name == name
        })
    }

    /// Children of `parent` in creation order.
    #[must_use]
    pub fn children(&self, parent: NodeId) -> &[NodeId] {
        &self.node(parent).children
    }

    /// Parent of `id`, `None` for the root.
    #[must_use]
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.node(id).parent
    }

    /// Name of `id` without index.
    #[must_use]
    pub fn name(&self, id: NodeId) -> &str {
        &self.node(id).name
    }

    /// Index of `id` among siblings of the same name.
    #[must_use]
    pub fn index(&self, id: NodeId) -> i32 {
        self.node(id).index
    }

    /// Whether attribute `attr` is set on `id`.
    #[must_use]
    pub fn attribute(&self, id: NodeId, attr: Attribute) -> bool {
        self.node(id).attributes & attr.0 != 0
    }

    /// Sets or clears attribute `attr` on `id`.
    pub fn set_attribute(&mut self, id: NodeId, attr: Attribute, on: bool) {
        let node = self.node_mut(id);
        if on {
            node.attributes |= attr.0;
        } else {
            node.attributes &= !attr.0;
        }
    }

    /// All attribute bits of `id`.
    #[must_use]
    pub fn attributes(&self, id: NodeId) -> u8 {
        self.node(id).attributes
    }

    /// Replaces all attribute bits of `id`.
    pub fn set_attributes(&mut self, id: NodeId, bits: u8) {
        self.node_mut(id).attributes = bits;
    }

    /// Whether `id` is tied to a model.
    #[must_use]
    pub fn is_tied(&self, id: NodeId) -> bool {
        self.node(id).tied.is_some()
    }
}

impl Node {
    fn new(name: String, index: i32, parent: Option<NodeId>) -> Self {
        Self {
            name,
            index,
            parent,
            children: Vec::new(),
            value: Value::None,
            attributes: Attribute::READ.0 | Attribute::WRITE.0,
            tied: None,
        }
    }
}

fn past_root() -> PathError {
    PathError {
        message: "Attempt to move past root with '..'".to_owned(),
    }
}

fn components_with_index(path: &str, index: Option<i32>) -> Result<Vec<Component>, PathError> {
    let mut components = parse_path(path)?;
    if let (Some(index), Some(Component::Child(_, last))) = (index, components.last_mut()) {
        *last = index;
    }
    Ok(components)
}
