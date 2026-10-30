// SPDX-License-Identifier: LGPL-2.1-or-later
//! The property tree in a snapshot: the shape the loads built, the nodes
//! created since, and every node's value.
//!
//! Node ids are arena slots, so a tree restored to the snapshot's shape
//! gives every node the id it had when the snapshot was taken, and the
//! ids models and functions hold stay valid. Nodes created after the
//! snapshot are dropped, and those created before it but missing here
//! are made again, in their order of creation.

use sha2::{Digest, Sha256};

use super::{Node, NodeId, PropertyTree, Value};
use crate::snapshot::{Checker, Reader, SnapshotError, Visit, Visitor, Writer, count_u32};

impl NodeId {
    /// The id of slot `slot`.
    pub(crate) fn from_u32(slot: u32) -> Self {
        Self(slot)
    }

    /// The slot as the snapshot stores it.
    pub(crate) const fn to_u32(self) -> u32 {
        self.0
    }
}

impl Value {
    fn tag(&self) -> u8 {
        match self {
            Self::None => 0,
            Self::Alias(_) => 1,
            Self::Bool(_) => 2,
            Self::Int(_) => 3,
            Self::Long(_) => 4,
            Self::Float(_) => 5,
            Self::Double(_) => 6,
            Self::String(_) => 7,
            Self::Unspecified(_) => 8,
        }
    }

    fn of_tag(tag: u8) -> Result<Self, SnapshotError> {
        Ok(match tag {
            0 => Self::None,
            1 => Self::Alias(NodeId(0)),
            2 => Self::Bool(false),
            3 => Self::Int(0),
            4 => Self::Long(0),
            5 => Self::Float(0.0),
            6 => Self::Double(0.0),
            7 => Self::String(String::new()),
            8 => Self::Unspecified(String::new()),
            other => {
                return Err(SnapshotError::InvalidValue {
                    what: "property value type",
                    value: u64::from(other),
                });
            }
        })
    }
}

/// The kind, then the value. A check meeting another kind than the
/// node's walks a fresh value of that kind in its place.
impl Visit for Value {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        let mut tag = self.tag();
        v.u8(&mut tag)?;
        if tag == self.tag() {
            return self.visit_payload(v);
        }
        let mut fresh = Self::of_tag(tag)?;
        if v.mutating() {
            *self = fresh;
            self.visit_payload(v)
        } else {
            fresh.visit_payload(v)
        }
    }
}

impl Value {
    fn visit_payload<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        match self {
            Self::None => Ok(()),
            Self::Alias(n) => n.visit(v),
            Self::Bool(b) => b.visit(v),
            Self::Int(i) => i.visit(v),
            Self::Long(l) => l.visit(v),
            Self::Float(f) => f.visit(v),
            Self::Double(d) => d.visit(v),
            Self::String(s) | Self::Unspecified(s) => s.visit(v),
        }
    }
}

impl PropertyTree {
    /// Nodes in the arena.
    pub(crate) fn node_count(&self) -> u32 {
        u32::try_from(self.nodes.len()).unwrap_or(u32::MAX)
    }

    /// SHA-256 of the first `load_nodes` nodes' structure: parent, index,
    /// name, tie and alias, but no value.
    pub(crate) fn shape_digest(&self, load_nodes: u32) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(load_nodes.to_le_bytes());
        for node in self.nodes.iter().take(load_nodes as usize) {
            let parent = node.parent.map_or(u32::MAX, |p| p.0);
            h.update(parent.to_le_bytes());
            h.update(node.index.to_le_bytes());
            h.update((node.name.len() as u64).to_le_bytes());
            h.update(node.name.as_bytes());
            let tied = match node.tied {
                None => 0u8,
                Some((read, write)) => 1 | u8::from(read) << 1 | u8::from(write) << 2,
            };
            h.update([tied]);
            let alias = match node.value {
                Value::Alias(target) => target.0,
                _ => u32::MAX,
            };
            h.update(alias.to_le_bytes());
        }
        h.finalize().into()
    }

    /// The nodes created after the first `load_nodes`: parent slot, index
    /// and name of each, in creation order.
    pub(crate) fn write_runtime_nodes(
        &self,
        w: &mut Writer<'_>,
        load_nodes: u32,
    ) -> Result<(), SnapshotError> {
        let runtime = self.nodes.get(load_nodes as usize..).unwrap_or_default();
        w.put(&count_u32(runtime.len(), "runtime nodes")?.to_le_bytes());
        for node in runtime {
            w.put(&node.parent.map_or(u32::MAX, |p| p.0).to_le_bytes());
            w.put(&node.index.to_le_bytes());
            w.put(&count_u32(node.name.len(), "runtime node name bytes")?.to_le_bytes());
            w.put(node.name.as_bytes());
        }
        Ok(())
    }

    /// Parses the records [`write_runtime_nodes`](Self::write_runtime_nodes)
    /// wrote, failing where [`read_runtime_nodes`](Self::read_runtime_nodes)
    /// would, and changes nothing: each parent must be a node made before
    /// the record's own.
    pub(crate) fn check_runtime_nodes(
        &self,
        r: &mut Reader<'_>,
        load_nodes: u32,
    ) -> Result<RuntimeNodes, SnapshotError> {
        if self.node_count() < load_nodes {
            return Err(SnapshotError::LoadNodes {
                expected: load_nodes,
                found: self.node_count(),
            });
        }
        let count = r.read_u32()?;
        let mut matches = self.node_count() == load_nodes.saturating_add(count);
        for i in 0..count {
            let (parent, index, name) = runtime_node(r)?;
            let before = load_nodes.saturating_add(i);
            if parent >= before {
                return Err(SnapshotError::RuntimeNode {
                    name: name.to_owned(),
                    parent,
                    nodes: before,
                });
            }
            matches = matches
                && self.nodes.get(before as usize).is_some_and(|n| {
                    n.parent.map_or(u32::MAX, |p| p.0) == parent
                        && n.index == index
                        && n.name == name
                });
        }
        Ok(RuntimeNodes {
            load_nodes,
            count,
            matches,
        })
    }

    /// Brings the nodes past the first `load_nodes` to those
    /// [`write_runtime_nodes`](Self::write_runtime_nodes) recorded. When
    /// they already match, nothing changes and nothing allocates.
    pub(crate) fn read_runtime_nodes(
        &mut self,
        r: &mut Reader<'_>,
        load_nodes: u32,
    ) -> Result<(), SnapshotError> {
        let mut check = r.clone();
        let runtime = self.check_runtime_nodes(&mut check, load_nodes)?;
        if runtime.matches {
            *r = check;
            return Ok(());
        }
        self.nodes.truncate(load_nodes as usize);
        for node in &mut self.nodes {
            node.children.retain(|c| c.0 < load_nodes);
        }
        r.read_u32()?;
        for _ in 0..runtime.count {
            let (parent, index, name) = runtime_node(r)?;
            if parent >= self.node_count() {
                return Err(SnapshotError::RuntimeNode {
                    name: name.to_owned(),
                    parent,
                    nodes: self.node_count(),
                });
            }
            self.add_child(NodeId(parent), name.to_owned(), index);
        }
        Ok(())
    }

    /// The lookup generation, then each node's attribute bits and value.
    pub(crate) fn visit_values<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        self.generation.visit(v)?;
        expect_nodes(v, self.nodes.len())?;
        self.nodes.iter_mut().try_for_each(|node| node.visit(v))
    }

    /// [`visit_values`](Self::visit_values) over the tree as
    /// [`read_runtime_nodes`](Self::read_runtime_nodes) would leave it
    /// with `runtime`, from [`check_runtime_nodes`](Self::check_runtime_nodes):
    /// a node it would make again, or make anew, is walked as a fresh
    /// one, and one it would drop is not walked.
    pub(crate) fn check_values(
        &mut self,
        c: &mut Checker<'_>,
        runtime: RuntimeNodes,
    ) -> Result<(), SnapshotError> {
        self.generation.visit(c)?;
        let kept = match runtime.matches {
            true => self.nodes.len(),
            false => runtime.load_nodes as usize,
        };
        let n = runtime.load_nodes as usize + runtime.count as usize;
        expect_nodes(c, n)?;
        self.nodes
            .iter_mut()
            .take(kept.min(n))
            .try_for_each(|node| node.visit(c))?;
        let mut fresh = Node::new(String::new(), 0, None);
        (kept.min(n)..n).try_for_each(|_| fresh.visit(c))
    }
}

/// What [`PropertyTree::check_runtime_nodes`] found.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeNodes {
    load_nodes: u32,
    count: u32,
    /// Whether the tree already has these nodes past the load's.
    matches: bool,
}

/// The node count, which must be `n`.
fn expect_nodes<V: Visitor>(v: &mut V, n: usize) -> Result<(), SnapshotError> {
    let found = v.len(n, "property nodes")?;
    if found != n {
        return Err(SnapshotError::Shape {
            what: "property nodes",
            expected: n as u64,
            found: found as u64,
        });
    }
    Ok(())
}

impl Visit for Node {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        self.attributes.visit(v)?;
        self.value.visit(v)
    }
}

fn runtime_node<'a>(r: &mut Reader<'a>) -> Result<(u32, i32, &'a str), SnapshotError> {
    let parent = r.read_u32()?;
    let index = r.read_u32()? as i32;
    let name = r.read_str()?;
    Ok((parent, index, name))
}
