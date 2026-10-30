// SPDX-License-Identifier: LGPL-2.1-or-later
//! Why a snapshot cannot be decoded or restored.

use thiserror::Error as ThisError;

/// The part of a [`Fingerprint`](super::Fingerprint) that differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FingerprintPart {
    /// The XML documents the loads resolved, byte for byte.
    Documents,
    /// The property tree as the loads left it: its nodes, their names,
    /// indices, parents, ties and aliases.
    Tree,
    /// The models' structure: how many components, engines, thrusters,
    /// tanks, contacts, gas cells and functions there are, and of which
    /// kinds.
    Models,
}

impl std::fmt::Display for FingerprintPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Documents => "loaded XML documents",
            Self::Tree => "load-time property tree",
            Self::Models => "model structure",
        })
    }
}

/// A snapshot that does not decode, or does not fit this executive.
#[derive(Debug, Clone, PartialEq, Eq, ThisError)]
#[non_exhaustive]
pub enum SnapshotError {
    /// The bytes do not start with [`MAGIC`](super::MAGIC).
    #[error("not a jsbsim snapshot: the first bytes are {found:02x?}")]
    BadMagic {
        /// The bytes found instead.
        found: Vec<u8>,
    },
    /// The snapshot was written in another layout.
    #[error("snapshot format version {found}, but this build reads version {expected}")]
    VersionMismatch {
        /// The snapshot's version.
        found: u32,
        /// [`FORMAT_VERSION`](super::FORMAT_VERSION).
        expected: u32,
    },
    /// The snapshot is of another model, or of one loaded another way.
    #[error("snapshot of another model: its {part} differ from this executive's")]
    FingerprintMismatch {
        /// The first part that differs, in [`FingerprintPart`] order.
        part: FingerprintPart,
    },
    /// The bytes end inside a value.
    #[error("snapshot truncated: {needed} more bytes needed at offset {offset} of {len}")]
    Truncated {
        /// Where the value starts.
        offset: usize,
        /// The bytes the value needs.
        needed: usize,
        /// The length of the snapshot.
        len: usize,
    },
    /// Bytes are left after the last value.
    #[error("snapshot has {extra} bytes after its last value")]
    TrailingBytes {
        /// How many.
        extra: usize,
    },
    /// A count the loaded model fixes differs from the snapshot's.
    #[error("snapshot has {found} {what}, but this executive has {expected}")]
    Shape {
        /// What is counted.
        what: &'static str,
        /// This executive's count.
        expected: u64,
        /// The snapshot's.
        found: u64,
    },
    /// A value no state can hold, such as a boolean that is neither 0
    /// nor 1.
    #[error("snapshot holds {value} for a {what}, which takes no such value")]
    InvalidValue {
        /// What was read.
        what: &'static str,
        /// The value.
        value: u64,
    },
    /// Text that is not UTF-8.
    #[error("snapshot holds a string that is not UTF-8 at offset {offset}")]
    InvalidText {
        /// Where the text starts.
        offset: usize,
    },
    /// A property node created after loading whose parent the snapshot
    /// does not hold.
    #[error(
        "snapshot's runtime property node {name:?} has parent slot {parent}, beyond its {nodes} nodes"
    )]
    RuntimeNode {
        /// The node's name.
        name: String,
        /// The parent's slot.
        parent: u32,
        /// The nodes before it.
        nodes: u32,
    },
    /// The executive has fewer property nodes than the snapshot says the
    /// loads made.
    #[error("snapshot needs {expected} load-time property nodes, but this executive has {found}")]
    LoadNodes {
        /// The snapshot's load-time node count.
        expected: u32,
        /// This executive's node count.
        found: u32,
    },
}
