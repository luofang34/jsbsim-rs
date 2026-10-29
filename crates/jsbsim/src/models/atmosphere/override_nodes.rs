// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGAtmosphere.{h,cpp} (Calculate: the
// override_*_node pointers).
//
//! The `atmosphere/override/*` nodes, found the way `FGAtmosphere`
//! finds them.
//!
//! `Calculate` looks each node up while its pointer is still null and
//! keeps it once found, so a node created at any time takes effect from
//! the next `Calculate`, and a found node is never looked up again. Here a
//! miss is retried only when the tree's lookup generation has moved,
//! which is when a lookup could first succeed; the result is the same
//! without walking the tree every frame.

use crate::properties::{Attribute, NodeId, PropertyTree};

/// The override nodes found so far.
#[derive(Debug, Clone, Copy, Default)]
pub struct OverrideNodes {
    /// `atmosphere/override/temperature`.
    pub temperature: Option<NodeId>,
    /// `atmosphere/override/pressure`.
    pub pressure: Option<NodeId>,
    /// `atmosphere/override/density`.
    pub density: Option<NodeId>,
    /// Tree generation of the last lookup; `None` before the first.
    looked_up_at: Option<u32>,
}

impl OverrideNodes {
    /// Looks up the nodes still missing, unless no node has been created
    /// since the last lookup.
    pub fn resolve(&mut self, tree: &PropertyTree) {
        let generation = tree.generation();
        if self.looked_up_at == Some(generation) {
            return;
        }
        self.looked_up_at = Some(generation);
        if self.temperature.is_some() && self.pressure.is_some() && self.density.is_some() {
            return;
        }
        let Some(parent) = tree
            .child(tree.root(), "atmosphere", 0)
            .and_then(|atmosphere| tree.child(atmosphere, "override", 0))
        else {
            return;
        };
        for (slot, name) in [
            (&mut self.temperature, "temperature"),
            (&mut self.pressure, "pressure"),
            (&mut self.density, "density"),
        ] {
            if slot.is_none() {
                *slot = tree
                    .child(parent, name, 0)
                    .filter(|&node| !tree.attribute(node, Attribute::REMOVED));
            }
        }
    }
}

crate::snapshot::visit_fields! { OverrideNodes { temperature, pressure, density, looked_up_at } }
