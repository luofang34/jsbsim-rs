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
//! value: a model that publishes state holds the [`SlotId`] of a tied node
//! and stores into it with [`PropertyTree::store`], which ignores the WRITE
//! attribute that keeps outside writers off read-only nodes.

mod conversion;
mod id;
mod naming;
mod node_map;
mod path;
mod snapshot;
mod value;

#[cfg(test)]
mod tests;

pub use id::NodeId;
#[cfg_attr(not(feature = "__oracle"), allow(unreachable_pub))]
pub use id::SlotId;
#[cfg_attr(not(feature = "__oracle"), allow(unreachable_pub))]
pub use naming::mk_property_name;
pub(crate) use node_map::NodeMap;
pub use path::PathError;
#[cfg_attr(not(feature = "__oracle"), allow(unreachable_pub))]
pub use value::Value;

use id::TreeId;
use path::{Component, parse_path};

/// SimGear node attribute bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(not(feature = "__oracle"), allow(unreachable_pub))]
pub struct Attribute(u8);

#[cfg_attr(not(feature = "__oracle"), allow(unreachable_pub))]
impl Attribute {
    /// Value may be read.
    pub const READ: Self = Self(1);
    /// Value may be written.
    pub const WRITE: Self = Self(2);
    /// Saved by archives.
    #[cfg_attr(not(feature = "__oracle"), allow(dead_code))]
    pub const ARCHIVE: Self = Self(4);
    /// Node was removed.
    pub const REMOVED: Self = Self(8);
    /// Reads are logged.
    pub const TRACE_READ: Self = Self(16);
    /// Writes are logged.
    pub const TRACE_WRITE: Self = Self(32);
    /// Saved by user archives.
    #[cfg_attr(not(feature = "__oracle"), allow(dead_code))]
    pub const USERARCHIVE: Self = Self(64);
    /// Preserved across resets.
    pub const PRESERVE: Self = Self(128);
}

/// Type of the value a node holds (`simgear::props::Type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(not(feature = "__oracle"), allow(dead_code, unreachable_pub))]
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
    parent: Option<SlotId>,
    children: Vec<SlotId>,
    value: Value,
    attributes: u8,
    /// READ and WRITE as they were when the node was tied; `Some` while
    /// tied.
    tied: Option<(bool, bool)>,
}

/// Arena of property nodes rooted at an unnamed node.
///
/// Relative paths start at the manager root ([`root`](Self::root)), which
/// is the true root unless the tree was made `for_instance`; a leading
/// `/` always starts at the true root, as SimGear's `getNode` does.
#[derive(Debug)]
#[cfg_attr(not(feature = "__oracle"), allow(unreachable_pub))]
pub struct PropertyTree {
    /// What sets this tree's [`NodeId`]s apart from any other's.
    id: TreeId,
    nodes: Vec<Node>,
    /// `FGPropertyManager`'s root node.
    base: SlotId,
    /// Changes whenever a path lookup could start resolving differently:
    /// a node is created, or one's REMOVED bit flips.
    generation: u32,
}

impl Default for PropertyTree {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for PropertyTree {
    /// A copy is a tree of its own: [`NodeId`]s of this one do not reach it.
    fn clone(&self) -> Self {
        Self {
            id: TreeId::mint(),
            nodes: self.nodes.clone(),
            base: self.base,
            generation: self.generation,
        }
    }
}

#[cfg_attr(not(feature = "__oracle"), allow(unreachable_pub))]
impl PropertyTree {
    /// Creates a tree holding only the root.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: TreeId::mint(),
            nodes: vec![Node::new(String::new(), 0, None)],
            base: SlotId(0),
            generation: 0,
        }
    }

    /// A tree whose manager root is `/fdm/jsbsim[index]`, as `FGFDMExec`
    /// makes its instance (`Root->getNode("fdm/jsbsim", IdFDM, true)`):
    /// absolute paths then reach nodes outside the instance.
    #[must_use]
    pub(crate) fn for_instance(index: i32) -> Self {
        let mut tree = Self::new();
        let fdm = tree.add_child(SlotId(0), "fdm".to_owned(), 0);
        tree.base = tree.add_child(fdm, "jsbsim".to_owned(), index);
        tree
    }

    /// The lookup generation: while it is unchanged, a path that resolved
    /// to no node still resolves to none, so a cached miss stays valid.
    #[must_use]
    pub(crate) fn generation(&self) -> u32 {
        self.generation
    }

    /// The manager root, where relative paths start
    /// (`FGPropertyManager::GetNode()`).
    #[must_use]
    pub fn root(&self) -> SlotId {
        self.base
    }

    /// The true root, where absolute paths start (`getRootNode`).
    #[must_use]
    pub const fn true_root() -> SlotId {
        SlotId(0)
    }

    /// The node `id`. A tree never removes nodes, so only an id from
    /// another tree can miss; it reads as [`DETACHED`].
    fn node(&self, id: SlotId) -> &Node {
        self.nodes.get(id.index()).unwrap_or(&DETACHED)
    }

    /// The node `id`; `None` only for an id from another tree, whose
    /// writes are dropped as [`DETACHED`] refuses them.
    fn node_mut(&mut self, id: SlotId) -> Option<&mut Node> {
        self.nodes.get_mut(id.index())
    }

    /// Finds the node at `path` relative to `from` (`getNode(path)`).
    pub fn find(&self, from: SlotId, path: &str) -> Result<Option<SlotId>, PathError> {
        self.find_indexed(from, path, None)
    }

    /// Like [`find`](Self::find) with the index of the last component
    /// replaced by `index` (`getNode(path, index)`).
    pub fn find_indexed(
        &self,
        from: SlotId,
        path: &str,
        index: Option<i32>,
    ) -> Result<Option<SlotId>, PathError> {
        let mut current = from;
        for component in parse_path(path, index)? {
            current = match component {
                Component::Root => Self::true_root(),
                Component::Current => current,
                Component::Parent => self.parent(current).ok_or_else(past_root)?,
                Component::Child(name, index) => match self.child(current, name, index) {
                    Some(child) => child,
                    None => return Ok(None),
                },
            };
        }
        Ok(self.live(current))
    }

    /// Finds or creates the node at `path` relative to `from`
    /// (`getNode(path, true)`).
    pub fn get_or_create(&mut self, from: SlotId, path: &str) -> Result<SlotId, PathError> {
        self.get_or_create_indexed(from, path, None)
    }

    /// Like [`get_or_create`](Self::get_or_create) with the index of the
    /// last component replaced by `index`. Fails for a `from` that another
    /// tree created, which has nowhere to attach new nodes.
    pub fn get_or_create_indexed(
        &mut self,
        from: SlotId,
        path: &str,
        index: Option<i32>,
    ) -> Result<SlotId, PathError> {
        if self.nodes.get(from.index()).is_none() {
            return Err(PathError {
                message: format!("node {} is not in this property tree", from.index()),
            });
        }
        let mut current = from;
        for component in parse_path(path, index)? {
            current = match component {
                Component::Root => Self::true_root(),
                Component::Current => current,
                Component::Parent => self.parent(current).ok_or_else(past_root)?,
                Component::Child(name, index) => match self.child(current, name, index) {
                    Some(child) => child,
                    None => self.add_child(current, name.to_owned(), index),
                },
            };
        }
        Ok(current)
    }

    /// Finds `path` from the root (`FGPropertyManager::GetNode`).
    pub fn get(&self, path: &str) -> Result<Option<SlotId>, PathError> {
        self.find(self.root(), path)
    }

    /// Whether `path` exists, ignoring a leading `-` as JSBSim does for
    /// negated inputs (`FGPropertyManager::HasNode`).
    pub fn has_node(&self, path: &str) -> Result<bool, PathError> {
        let path = path.strip_prefix('-').unwrap_or(path);
        Ok(self.get(path)?.is_some())
    }

    fn live(&self, id: SlotId) -> Option<SlotId> {
        (self.node(id).attributes & Attribute::REMOVED.0 == 0).then_some(id)
    }

    fn add_child(&mut self, parent: SlotId, name: String, index: i32) -> SlotId {
        let id = SlotId(u32::try_from(self.nodes.len()).unwrap_or(u32::MAX));
        if let Some(parent) = self.node_mut(parent) {
            parent.children.push(id);
        }
        self.nodes.push(Node::new(name, index, Some(parent)));
        self.generation = self.generation.wrapping_add(1);
        id
    }

    /// Child of `parent` named `name` with `index` (`getChild`).
    #[must_use]
    pub(crate) fn child(&self, parent: SlotId, name: &str, index: i32) -> Option<SlotId> {
        self.node(parent).children.iter().copied().find(|&c| {
            let node = self.node(c);
            node.index == index && node.name == name
        })
    }

    /// Children of `parent` in creation order.
    #[must_use]
    #[cfg_attr(not(feature = "__oracle"), allow(dead_code))]
    pub fn children(&self, parent: SlotId) -> &[SlotId] {
        &self.node(parent).children
    }

    /// Parent of `id`, `None` for the root.
    #[must_use]
    pub(crate) fn parent(&self, id: SlotId) -> Option<SlotId> {
        self.node(id).parent
    }

    /// Name of `id` without index.
    #[must_use]
    pub(crate) fn name(&self, id: SlotId) -> &str {
        &self.node(id).name
    }

    /// Index of `id` among siblings of the same name.
    #[must_use]
    pub(crate) fn index(&self, id: SlotId) -> i32 {
        self.node(id).index
    }

    /// Whether attribute `attr` is set on `id`.
    #[must_use]
    pub fn attribute(&self, id: SlotId, attr: Attribute) -> bool {
        self.node(id).attributes & attr.0 != 0
    }

    /// Sets or clears attribute `attr` on `id`.
    pub fn set_attribute(&mut self, id: SlotId, attr: Attribute, on: bool) {
        let bits = self.node(id).attributes;
        self.set_attributes(id, if on { bits | attr.0 } else { bits & !attr.0 });
    }

    /// Replaces all attribute bits of `id`.
    pub(crate) fn set_attributes(&mut self, id: SlotId, bits: u8) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        let removed_changed = (node.attributes ^ bits) & Attribute::REMOVED.0 != 0;
        node.attributes = bits;
        if removed_changed {
            self.generation = self.generation.wrapping_add(1);
        }
    }

    /// Whether `id` is tied to a model.
    #[must_use]
    pub fn is_tied(&self, id: SlotId) -> bool {
        self.node(id).tied.is_some()
    }
}

/// What an id this tree did not create reads as: a removed, childless node
/// with neither READ nor WRITE, so reads give the type's default (0, false,
/// "") as SimGear's getters do for an unreadable node, writes are refused,
/// and path lookups through it find nothing.
static DETACHED: Node = Node {
    name: String::new(),
    index: 0,
    parent: None,
    children: Vec::new(),
    value: Value::None,
    attributes: Attribute::REMOVED.0,
    tied: None,
};

impl Node {
    fn new(name: String, index: i32, parent: Option<SlotId>) -> Self {
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
