// SPDX-License-Identifier: LGPL-2.1-or-later
//! Handles to property nodes. Inside the crate a node is a [`SlotId`], an
//! index into its tree's arena. Across the public API it is a [`NodeId`],
//! the slot together with the [`NodeKey`] of the node there, because a bare
//! index would read whichever node happens to sit at that index: in another
//! tree, or in the same tree after a snapshot restore replaced its runtime
//! nodes.

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

/// A node's name, index and ancestors, hashed (FNV-1a, 64 bits) so that a
/// [`NodeId`] can check it still names the node it was made for. The same
/// path gives the same key in every tree and on every target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct NodeKey(u64);

impl NodeKey {
    /// The key of the root, which has no name.
    pub(super) const ROOT: Self = Self(0xcbf2_9ce4_8422_2325);

    /// The key of child `name[index]` of the node keyed `self`.
    pub(super) fn child(self, name: &str, index: i32) -> Self {
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        // 0xff never occurs in UTF-8, so it ends the name unambiguously.
        let bytes = self
            .0
            .to_le_bytes()
            .into_iter()
            .chain(name.bytes())
            .chain([0xff])
            .chain(index.to_le_bytes());
        Self(bytes.fold(Self::ROOT.0, |h, b| (h ^ u64::from(b)).wrapping_mul(PRIME)))
    }
}

/// Handle to a property node, from [`Fdm::node`](crate::Fdm::node): the
/// node of one path at one slot of the property arena. It works on any
/// [`Fdm`](crate::Fdm) that holds that node there, such as a second
/// executive of the same model, and is refused with
/// [`Error::StaleNode`](crate::Error::StaleNode) by one that does not,
/// such as an executive a snapshot restore took the node from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId {
    slot: SlotId,
    key: NodeKey,
}

impl NodeId {
    /// The slot the handle names, whatever node is there now.
    pub(crate) fn slot(self) -> SlotId {
        self.slot
    }
}

#[cfg_attr(not(feature = "__oracle"), allow(unreachable_pub))]
impl PropertyTree {
    /// The handle by which callers outside the crate name `slot`.
    #[must_use]
    pub fn node_id(&self, slot: SlotId) -> NodeId {
        NodeId {
            slot,
            key: self.node(slot).key,
        }
    }

    /// The slot `id` names, `None` unless that slot holds the node `id` was
    /// made for: a node of the same path, in this tree or another.
    #[must_use]
    pub fn slot_of(&self, id: NodeId) -> Option<SlotId> {
        let node = self.nodes.get(id.slot.index())?;
        (node.key == id.key).then_some(id.slot)
    }

    /// The path of the node at `slot`, `None` past the end.
    pub(crate) fn path_at(&self, slot: SlotId) -> Option<String> {
        (slot.index() < self.nodes.len()).then(|| self.path(slot, true))
    }
}
