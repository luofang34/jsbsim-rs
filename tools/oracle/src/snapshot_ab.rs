// SPDX-License-Identifier: LGPL-2.1-or-later
//! A/B checks of [`Fdm::snapshot_into`] and [`Fdm::restore`]: a run is
//! snapshotted, continued to give trace A, restored and continued again
//! to give trace B, and the two must agree bit for bit.
//!
//! A trace holds, after every frame, the bits of every property as
//! [`Fdm::read_node`] reads it (getters and functions evaluated), and the
//! SHA-256 of a snapshot of the whole state, which covers what no
//! property shows. Reading a function can draw from a random generator,
//! so the reads are part of the run; both traces read the same nodes in
//! the same order, so they perturb both alike.

use jsbsim::Error;
use jsbsim::Fdm;
use jsbsim::oracle_support::properties::{NodeId, PropertyTree};
use jsbsim::snapshot::Snapshot;
use sha2::{Digest, Sha256};

pub mod cli;
#[cfg(test)]
mod tests;

/// What the host does before each frame of a trace, such as creating a
/// property; it must do the same in A and B.
pub type Host<'a> = &'a mut dyn FnMut(&mut Fdm) -> Result<(), Error>;

/// Every node of `tree` in depth-first creation order, from the true
/// root.
#[must_use]
pub fn nodes(tree: &PropertyTree) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack = vec![tree.true_root()];
    while let Some(node) = stack.pop() {
        out.push(node);
        stack.extend(tree.children(node).iter().rev());
    }
    out
}

/// One frame of a trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Each node's path and the bits of its value; an evaluation that
    /// fails reads as `u64::MAX`.
    pub values: Vec<(String, u64)>,
    /// SHA-256 of a snapshot taken after the frame.
    pub state: [u8; 32],
}

impl Frame {
    /// SHA-256 of the frame: each path and value, then the state digest.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        for (path, bits) in &self.values {
            h.update((path.len() as u64).to_le_bytes());
            h.update(path.as_bytes());
            h.update(bits.to_le_bytes());
        }
        h.update(self.state);
        h.finalize().into()
    }
}

/// Runs `frames` frames after `host` acts on each, recording each, or
/// fewer if the run ends.
pub fn record(fdm: &mut Fdm, frames: u32, host: Host<'_>) -> Result<Vec<Frame>, Error> {
    let mut out = Vec::new();
    let mut snapshot = Snapshot::new();
    for _ in 0..frames {
        host(fdm)?;
        let running = fdm.run()?;
        let mut values = Vec::new();
        for node in nodes(fdm.tree()) {
            let path = fdm.tree().path(node, false);
            let bits = fdm.read_node(node).map_or(u64::MAX, f64::to_bits);
            values.push((path, bits));
        }
        fdm.snapshot_into(&mut snapshot)?;
        out.push(Frame {
            values,
            state: Sha256::digest(snapshot.as_bytes()).into(),
        });
        if !running {
            break;
        }
    }
    Ok(out)
}

/// Where two traces first differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    /// One trace is longer.
    Length {
        /// Frames of A.
        a: usize,
        /// Frames of B.
        b: usize,
    },
    /// The nodes differ.
    Nodes {
        /// The frame, from 0.
        frame: usize,
    },
    /// A property's bits differ.
    Value {
        /// The frame, from 0.
        frame: usize,
        /// The property.
        path: String,
        /// Its bits in A.
        a: u64,
        /// Its bits in B.
        b: u64,
    },
    /// The snapshots differ though every property agrees.
    State {
        /// The frame, from 0.
        frame: usize,
    },
}

/// The first difference between traces `a` and `b`.
#[must_use]
pub fn compare(a: &[Frame], b: &[Frame]) -> Option<Mismatch> {
    if a.len() != b.len() {
        return Some(Mismatch::Length {
            a: a.len(),
            b: b.len(),
        });
    }
    for (frame, (fa, fb)) in a.iter().zip(b).enumerate() {
        if fa.values.len() != fb.values.len() {
            return Some(Mismatch::Nodes { frame });
        }
        for ((pa, va), (pb, vb)) in fa.values.iter().zip(&fb.values) {
            if pa != pb {
                return Some(Mismatch::Nodes { frame });
            }
            if va != vb {
                return Some(Mismatch::Value {
                    frame,
                    path: pa.clone(),
                    a: *va,
                    b: *vb,
                });
            }
        }
        if fa.state != fb.state {
            return Some(Mismatch::State { frame });
        }
    }
    None
}

/// What [`ab`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbOutcome {
    /// Frames each trace holds.
    pub frames: usize,
    /// The first difference between A and B, if any.
    pub mismatch: Option<Mismatch>,
    /// Whether a snapshot taken right after the restore has the bytes of
    /// the one restored.
    pub resnapshot_identical: bool,
    /// The snapshot's size, bytes.
    pub bytes: usize,
}

/// Snapshots `fdm`, records `frames` frames (A), restores, records them
/// again (B) and compares.
pub fn ab(fdm: &mut Fdm, frames: u32) -> Result<AbOutcome, Error> {
    ab_with(fdm, None, frames, &mut |_| Ok(()))
}

/// [`ab`] with the host acting before every frame, and B recorded on
/// `fresh`, an executive loaded as `fdm` was, when given.
pub fn ab_with(
    fdm: &mut Fdm,
    fresh: Option<&mut Fdm>,
    frames: u32,
    host: Host<'_>,
) -> Result<AbOutcome, Error> {
    let mut snapshot = Snapshot::new();
    fdm.snapshot_into(&mut snapshot)?;
    let a = record(fdm, frames, host)?;
    let target = match fresh {
        Some(other) => other,
        None => fdm,
    };
    target.restore(&snapshot)?;
    let mut again = Snapshot::new();
    target.snapshot_into(&mut again)?;
    let b = record(target, frames, host)?;
    Ok(AbOutcome {
        frames: a.len(),
        mismatch: compare(&a, &b),
        resnapshot_identical: again == snapshot,
        bytes: snapshot.as_bytes().len(),
    })
}
