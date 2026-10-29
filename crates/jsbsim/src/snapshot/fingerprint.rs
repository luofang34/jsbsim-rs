// SPDX-License-Identifier: LGPL-2.1-or-later
//! What identifies a loaded model: its documents' bytes, the property
//! tree the loads built, and the models' structure, each hashed with
//! SHA-256.
//!
//! No path enters it: a native build, a WASI build with its preopened
//! directories and a browser fetching from a CDN load the same model
//! from different places, and must agree.

use std::cell::RefCell;

use sha2::{Digest, Sha256};

use super::{FORMAT_VERSION, FingerprintPart, HEADER_LEN, MAGIC, Reader, SnapshotError, Writer};
use crate::{Error, Resolver};

/// The identity of a loaded model that a snapshot must match to restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint {
    /// SHA-256 over every document the loads resolved, in load order,
    /// each as its length (`u64`, little-endian) and bytes.
    pub documents: [u8; 32],
    /// SHA-256 over the property nodes the loads left, in node order:
    /// parent slot, index, name, whether tied, alias target.
    pub tree: [u8; 32],
    /// SHA-256 over the models' structural counts and kinds, in the order
    /// a snapshot visits them.
    pub models: [u8; 32],
    /// The property nodes the loads left; nodes past them were created
    /// while running and are carried by name.
    pub load_nodes: u32,
}

impl Fingerprint {
    /// The first part in which `self` and `other` differ.
    #[must_use]
    pub fn first_difference(&self, other: &Self) -> Option<FingerprintPart> {
        if self.documents != other.documents {
            Some(FingerprintPart::Documents)
        } else if self.tree != other.tree || self.load_nodes != other.load_nodes {
            Some(FingerprintPart::Tree)
        } else if self.models != other.models {
            Some(FingerprintPart::Models)
        } else {
            None
        }
    }

    /// The fingerprint as the header holds it: three digests and the node
    /// count.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 100] {
        let mut out = [0; 100];
        for (chunk, part) in out
            .chunks_mut(32)
            .zip([&self.documents, &self.tree, &self.models])
        {
            if chunk.len() == 32 {
                chunk.copy_from_slice(part);
            }
        }
        if let Some(tail) = out.get_mut(96..) {
            tail.copy_from_slice(&self.load_nodes.to_le_bytes());
        }
        out
    }

    pub(crate) fn write_header(&self, w: &mut Writer<'_>) {
        w.put(&MAGIC);
        w.put(&FORMAT_VERSION.to_le_bytes());
        w.put(&self.to_bytes());
    }

    /// Checks the magic and version of `bytes`, and reads the fingerprint.
    pub(crate) fn read_header(bytes: &[u8]) -> Result<Self, SnapshotError> {
        let mut r = Reader::new(bytes, 0);
        let magic = r.take(MAGIC.len()).map_err(|_| SnapshotError::BadMagic {
            found: bytes.to_vec(),
        })?;
        if magic != MAGIC {
            return Err(SnapshotError::BadMagic {
                found: magic.to_vec(),
            });
        }
        let found = r.read_u32()?;
        if found != FORMAT_VERSION {
            return Err(SnapshotError::VersionMismatch {
                found,
                expected: FORMAT_VERSION,
            });
        }
        let mut digest = || -> Result<[u8; 32], SnapshotError> {
            let mut out = [0; 32];
            out.copy_from_slice(r.take(32)?);
            Ok(out)
        };
        let (documents, tree, models) = (digest()?, digest()?, digest()?);
        let load_nodes = r.read_u32()?;
        Ok(Self {
            documents,
            tree,
            models,
            load_nodes,
        })
    }

    /// The body's reader, after the header.
    pub(crate) fn body(bytes: &[u8]) -> Reader<'_> {
        Reader::new(bytes, HEADER_LEN)
    }
}

/// What the loads so far contribute to the fingerprint.
#[derive(Clone, Default)]
pub(crate) struct LoadRecord {
    documents: Sha256,
    load_nodes: u32,
    /// The fingerprint, once computed for the current load.
    cached: Option<Fingerprint>,
}

impl std::fmt::Debug for LoadRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadRecord")
            .field("load_nodes", &self.load_nodes)
            .field("cached", &self.cached)
            .finish_non_exhaustive()
    }
}

impl LoadRecord {
    /// A load has finished: its documents' digest, and the tree's node
    /// count after it.
    pub(crate) fn loaded(&mut self, documents: [u8; 32], nodes: u32) {
        self.documents.update(documents);
        self.load_nodes = nodes;
        self.cached = None;
    }

    /// A load that read no document changed the structure.
    pub(crate) fn structure_changed(&mut self, nodes: u32) {
        self.load_nodes = nodes;
        self.cached = None;
    }

    pub(crate) fn load_nodes(&self) -> u32 {
        self.load_nodes
    }

    pub(crate) fn cached(&self) -> Option<Fingerprint> {
        self.cached
    }

    pub(crate) fn documents(&self) -> [u8; 32] {
        self.documents.clone().finalize().into()
    }

    pub(crate) fn cache(&mut self, fingerprint: Fingerprint) {
        self.cached = Some(fingerprint);
    }
}

/// A resolver that hashes the bytes of every document it resolves.
pub(crate) struct HashingResolver<'a> {
    inner: &'a dyn Resolver,
    hasher: RefCell<Sha256>,
}

impl<'a> HashingResolver<'a> {
    pub(crate) fn new(inner: &'a dyn Resolver) -> Self {
        Self {
            inner,
            hasher: RefCell::new(Sha256::new()),
        }
    }

    pub(crate) fn finish(self) -> [u8; 32] {
        self.hasher.into_inner().finalize().into()
    }
}

impl Resolver for HashingResolver<'_> {
    fn resolve(&self, path: &str) -> Result<Vec<u8>, Error> {
        let bytes = self.inner.resolve(path)?;
        let mut hasher = self.hasher.borrow_mut();
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
        Ok(bytes)
    }

    fn exists(&self, path: &str) -> bool {
        self.inner.exists(path)
    }
}
