// SPDX-License-Identifier: LGPL-2.1-or-later
//! `Fdm::restore` is atomic: a snapshot with a byte changed, cut short or
//! with a byte appended either restores, or fails and leaves the
//! executive exactly as it was: its snapshot byte for byte, its
//! fingerprint and its property nodes.
//!
//! Each target runs into a state other than the snapshot's, so a write
//! made before the error would show. The pairs cover snapshots holding
//! more property nodes created while running than the target and fewer,
//! a random generator only one side has made, and friction constraints on
//! one side alone. Debug builds restore slowly, so every byte is changed
//! through the header, the runtime nodes' records and the first property
//! values, and through the last models' state; between them, one in a
//! stride.

use std::path::{Path, PathBuf};

use jsbsim::fdm::Fdm;
use jsbsim::properties::PropertyTree;
use jsbsim::snapshot::{HEADER_LEN, Snapshot, SnapshotError};
use jsbsim::{Error, FileResolver};

#[cfg(test)]
fn oracle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

#[cfg(test)]
fn run(fdm: &mut Fdm, frames: u32) {
    for _ in 0..frames {
        assert!(fdm.run().unwrap());
    }
}

#[cfg(test)]
fn ball(frames: u32, created: &[&str]) -> Fdm {
    let mut fdm = Fdm::new().unwrap();
    fdm.load_script(
        &FileResolver::new(oracle_dir().join("inputs")),
        "scripts/ball.xml",
    )
    .unwrap();
    fdm.run_ic().unwrap();
    run(&mut fdm, frames);
    for (i, path) in created.iter().enumerate() {
        fdm.set_property(path, i as f64 + 0.5).unwrap();
    }
    fdm
}

/// The c172x at rest on the runway, or lifted `altitude_agl_ft` above
/// it.
#[cfg(test)]
fn c172x(altitude_agl_ft: Option<f64>, frames: u32) -> Fdm {
    let mut fdm = Fdm::new().unwrap();
    fdm.load_script(
        &FileResolver::new(oracle_dir().join("inputs")),
        "scripts/c172_runway_at_rest_cg_shift.xml",
    )
    .unwrap();
    if let Some(altitude) = altitude_agl_ft {
        fdm.set_property("ic/h-agl-ft", altitude).unwrap();
    }
    fdm.run_ic().unwrap();
    run(&mut fdm, frames);
    fdm
}

/// The nodes reachable from the root, which are all the tree's.
#[cfg(test)]
fn node_count(tree: &PropertyTree) -> usize {
    let mut pending = vec![tree.true_root()];
    let mut count = 0;
    while let Some(id) = pending.pop() {
        count += 1;
        pending.extend_from_slice(tree.children(id));
    }
    count
}

#[cfg(test)]
fn snapshot(fdm: &mut Fdm) -> Snapshot {
    let mut s = Snapshot::new();
    fdm.snapshot_into(&mut s).unwrap();
    s
}

/// Restores corrupted copies of a snapshot onto a target and checks that
/// each failure left it as `reference` holds it.
#[cfg(test)]
struct Sweep<'a> {
    target: &'a mut Fdm,
    reference: Snapshot,
    fingerprint: jsbsim::snapshot::Fingerprint,
    nodes: usize,
    probe: Snapshot,
    refused: usize,
    restored: usize,
    malformed_header: usize,
}

#[cfg(test)]
impl<'a> Sweep<'a> {
    fn new(target: &'a mut Fdm) -> Self {
        let reference = snapshot(target);
        let fingerprint = target.fingerprint().unwrap();
        let nodes = node_count(target.tree());
        Self {
            target,
            reference,
            fingerprint,
            nodes,
            probe: Snapshot::new(),
            refused: 0,
            restored: 0,
            malformed_header: 0,
        }
    }

    /// Restores `bytes`; the error, if it was refused.
    fn restore(&mut self, bytes: &[u8], what: &str) -> Option<Error> {
        let Ok(corrupted) = Snapshot::from_bytes(bytes) else {
            self.malformed_header += 1;
            return None;
        };
        match self.target.restore(&corrupted) {
            Ok(()) => {
                self.restored += 1;
                self.target.restore(&self.reference).unwrap();
                None
            }
            Err(e) => {
                self.refused += 1;
                self.target.snapshot_into(&mut self.probe).unwrap();
                assert!(
                    self.probe == self.reference,
                    "{what}: the refused restore ({e}) changed the state"
                );
                assert_eq!(
                    self.target.fingerprint().unwrap(),
                    self.fingerprint,
                    "{what}"
                );
                assert_eq!(node_count(self.target.tree()), self.nodes, "{what}");
                Some(e)
            }
        }
    }

    /// Every offset of `s` in the first `head` bytes and the last `tail`,
    /// and every `stride`th between, set to each of a few patterns; `s`
    /// cut to each of those lengths; and one byte appended.
    fn run(&mut self, s: &Snapshot, head: usize, tail: usize, stride: usize) {
        assert!(
            *s != self.reference,
            "the target is in the snapshot's state"
        );
        let bytes = s.as_bytes().to_vec();
        let mut corrupted = bytes.clone();
        let offsets: Vec<usize> = (0..bytes.len())
            .filter(|&at| {
                at < head || at + tail >= bytes.len() || (at - head).is_multiple_of(stride)
            })
            .collect();
        for &at in &offsets {
            let byte = bytes[at];
            for pattern in [byte ^ 0xff, byte ^ 0x01, 0x00, 0xff] {
                if pattern == byte {
                    continue;
                }
                corrupted[at] = pattern;
                self.restore(&corrupted, &format!("byte {at} set to {pattern:#04x}"));
            }
            corrupted[at] = byte;
        }
        for &len in &offsets {
            let what = format!("cut to {len} bytes");
            let refused = self.restore(&bytes[..len], &what);
            assert!(len < HEADER_LEN || refused.is_some(), "{what}: restored");
        }
        corrupted.push(0);
        assert_eq!(
            self.restore(&corrupted, "one byte appended"),
            Some(Error::Snapshot(SnapshotError::TrailingBytes { extra: 1 }))
        );
        assert!(self.refused > 0 && self.restored > 0 && self.malformed_header > 0);
        self.target.restore(s).unwrap();
    }
}

/// The end of the runtime property nodes' records, which follow the
/// header as their count, then each node's parent, index and name.
#[cfg(test)]
fn runtime_nodes_end(bytes: &[u8]) -> usize {
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let mut at = HEADER_LEN + 4;
    for _ in 0..u32_at(HEADER_LEN) {
        at += 12 + u32_at(at + 8);
    }
    at
}

/// Bytes of property values corrupted at every offset after the records,
/// and bytes of the last models' state before the end.
#[cfg(test)]
const FIRST_VALUES: usize = 320;
#[cfg(test)]
const LAST_STATE: usize = 96;

/// The ball, with runtime property nodes on both sides, and the
/// executive's turbulence generator made on one.
#[test]
fn a_refused_ball_snapshot_leaves_the_executive_untouched() {
    let mut more = ball(80, &["test/a", "test/b[2]", "test/c/d"]);
    more.set_property("atmosphere/randomseed", 7.0).unwrap();
    let mut fewer = ball(30, &["test/e"]);
    assert!(more.models.winds.generator.is_some());
    assert!(fewer.models.winds.generator.is_none());
    assert!(node_count(more.tree()) > node_count(fewer.tree()));
    let (s_more, s_fewer) = (snapshot(&mut more), snapshot(&mut fewer));
    for (target, s) in [(&mut fewer, &s_more), (&mut more, &s_fewer)] {
        let head = runtime_nodes_end(s.as_bytes()) + FIRST_VALUES;
        Sweep::new(target).run(s, head, LAST_STATE, 61);
    }
}

/// The c172x at rest on its gear and in the air: only the first holds
/// friction constraints.
#[test]
fn a_refused_c172x_snapshot_leaves_the_executive_untouched() {
    let mut ground = c172x(None, 60);
    let mut air = c172x(Some(3000.0), 20);
    assert!(!ground.models.ground_reactions.multipliers.is_empty());
    assert!(air.models.ground_reactions.multipliers.is_empty());
    let (s_ground, s_air) = (snapshot(&mut ground), snapshot(&mut air));
    for (target, s) in [(&mut air, &s_ground), (&mut ground, &s_air)] {
        let head = runtime_nodes_end(s.as_bytes()) + FIRST_VALUES;
        Sweep::new(target).run(s, head, LAST_STATE, 211);
    }
}
