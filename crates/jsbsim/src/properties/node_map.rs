// SPDX-License-Identifier: LGPL-2.1-or-later
//! A map keyed by [`NodeId`], stored as a slot table.
//!
//! Node ids are dense indices into the tree's arena, so a lookup is one
//! index instead of a tree search, and iterating in slot order visits keys
//! in ascending id order, as an ordered map would.

use super::NodeId;

/// Values by node, one optional slot per node id up to the largest key.
#[derive(Debug, Clone)]
pub struct NodeMap<T> {
    slots: Vec<Option<T>>,
}

impl<T> Default for NodeMap<T> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<T> NodeMap<T> {
    /// An empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The value of `node`, if any.
    #[must_use]
    pub fn get(&self, node: NodeId) -> Option<&T> {
        self.slots.get(node.slot()).and_then(Option::as_ref)
    }

    /// Whether `node` has a value.
    #[must_use]
    pub fn contains(&self, node: NodeId) -> bool {
        self.get(node).is_some()
    }

    /// Sets the value of `node`, returning the one it replaces.
    pub fn insert(&mut self, node: NodeId, value: T) -> Option<T> {
        let slot = node.slot();
        if self.slots.len() <= slot {
            self.slots.resize_with(slot + 1, || None);
        }
        self.slots.get_mut(slot).and_then(|s| s.replace(value))
    }

    /// Removes the value of `node`, returning it.
    pub fn remove(&mut self, node: NodeId) -> Option<T> {
        self.slots.get_mut(node.slot()).and_then(Option::take)
    }

    /// Number of slots: every key is below [`NodeId`] of this index.
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// The entry in slot `slot`, if it holds a value.
    #[must_use]
    pub fn entry(&self, slot: usize) -> Option<(NodeId, &T)> {
        let value = self.slots.get(slot)?.as_ref()?;
        Some((NodeId::from_slot(slot)?, value))
    }

    /// Entries in ascending node order.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &T)> {
        (0..self.slots.len()).filter_map(|slot| self.entry(slot))
    }

    /// Whether no node has a value.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }
}
