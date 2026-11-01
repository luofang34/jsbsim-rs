// SPDX-License-Identifier: LGPL-2.1-or-later
//! A map keyed by [`SlotId`], stored as a slot table.
//!
//! Node ids are dense indices into the tree's arena, so a lookup is one
//! index instead of a tree search, and iterating in slot order visits keys
//! in ascending id order, as an ordered map would.

use super::SlotId;

/// Values by node, one optional slot per node id up to the largest key.
#[derive(Debug, Clone)]
pub(crate) struct NodeMap<T> {
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
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The value of `node`, if any.
    #[must_use]
    pub(crate) fn get(&self, node: SlotId) -> Option<&T> {
        self.slots.get(node.index()).and_then(Option::as_ref)
    }

    /// Whether `node` has a value.
    #[must_use]
    pub(crate) fn contains(&self, node: SlotId) -> bool {
        self.get(node).is_some()
    }

    /// Sets the value of `node`, returning the one it replaces.
    pub(crate) fn insert(&mut self, node: SlotId, value: T) -> Option<T> {
        let slot = node.index();
        if self.slots.len() <= slot {
            self.slots.resize_with(slot + 1, || None);
        }
        self.slots.get_mut(slot).and_then(|s| s.replace(value))
    }

    /// Removes the value of `node`, returning it.
    pub(crate) fn remove(&mut self, node: SlotId) -> Option<T> {
        self.slots.get_mut(node.index()).and_then(Option::take)
    }

    /// Number of slots: every key is below [`SlotId`] of this index.
    #[must_use]
    pub(crate) fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// The entry in slot `slot`, if it holds a value.
    #[must_use]
    pub(crate) fn entry(&self, slot: usize) -> Option<(SlotId, &T)> {
        let value = self.slots.get(slot)?.as_ref()?;
        Some((SlotId::from_index(slot)?, value))
    }

    /// Entries in ascending node order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (SlotId, &T)> {
        (0..self.slots.len()).filter_map(|slot| self.entry(slot))
    }

    /// Whether no node has a value.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }
}
