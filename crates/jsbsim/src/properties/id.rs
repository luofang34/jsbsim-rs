// SPDX-License-Identifier: LGPL-2.1-or-later
//! Handles to property nodes. Inside the crate a node is a [`SlotId`], an
//! index into its tree's arena. Across the public API it is a [`NodeId`],
//! the slot stamped with the [`TreeId`] of the tree it came from, because a
//! bare index would read whichever node happens to sit at that index in
//! another tree.

use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU32, Ordering};

use super::PropertyTree;

/// Index of a node in the arena of its [`PropertyTree`]. A snapshot stores
/// these, so they carry over to any executive of the same fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(not(feature = "__oracle"), allow(unreachable_pub))]
pub struct SlotId(pub(super) u32);

impl SlotId {
    /// Index of the node in the tree's arena.
    pub(super) fn index(self) -> usize {
        self.0 as usize
    }

    pub(super) fn from_index(index: usize) -> Option<Self> {
        u32::try_from(index).ok().map(Self)
    }
}

/// Which [`PropertyTree`] a [`NodeId`] came from. It tells live trees apart
/// and means nothing else, so it stays out of fingerprints and snapshots,
/// which must match across runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct TreeId(NonZeroU32);

static NEXT_TREE_ID: AtomicU32 = AtomicU32::new(1);

impl TreeId {
    /// The counter's value after `id`, which skips 0 as it wraps.
    pub(super) fn after(id: u32) -> u32 {
        match id.wrapping_add(1) {
            0 => 1,
            next => next,
        }
    }

    /// An id no other tree has, until the counter wraps after 2^32 trees.
    pub(super) fn mint() -> Self {
        let id = NEXT_TREE_ID.update(Ordering::Relaxed, Ordering::Relaxed, Self::after);
        // The counter never holds 0.
        Self(NonZeroU32::new(id).unwrap_or(NonZeroU32::MIN))
    }
}

/// Handle to a node of one [`Fdm`](crate::Fdm)'s property tree, from
/// [`Fdm::node`](crate::Fdm::node).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId {
    tree: TreeId,
    slot: SlotId,
}

const _: () = assert!(size_of::<Option<NodeId>>() == 8);

#[cfg_attr(not(feature = "__oracle"), allow(unreachable_pub))]
impl PropertyTree {
    /// The handle by which callers outside the crate name `slot`.
    #[must_use]
    pub fn node_id(&self, slot: SlotId) -> NodeId {
        NodeId {
            tree: self.id,
            slot,
        }
    }

    /// The slot `id` names, `None` when `id` came from another tree.
    #[must_use]
    pub fn slot_of(&self, id: NodeId) -> Option<SlotId> {
        (id.tree == self.id).then_some(id.slot)
    }
}
