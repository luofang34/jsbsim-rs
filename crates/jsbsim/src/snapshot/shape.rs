// SPDX-License-Identifier: LGPL-2.1-or-later
//! The fingerprint's model-structure part: a walk that hashes only the
//! counts and kinds the loaded model fixes.

use sha2::{Digest, Sha256};

use super::{SnapshotError, Visitor};

/// Hashes each structural count and variant kind a walk meets, in order,
/// and none of the values.
#[derive(Default)]
pub(crate) struct ShapeHasher {
    hasher: Sha256,
}

impl ShapeHasher {
    pub(crate) fn finish(self) -> [u8; 32] {
        self.hasher.finalize().into()
    }

    fn label(&mut self, tag: u8, name: &str) {
        self.hasher.update([tag]);
        self.hasher.update((name.len() as u64).to_le_bytes());
        self.hasher.update(name.as_bytes());
    }
}

impl Visitor for ShapeHasher {
    fn loading(&self) -> bool {
        false
    }

    fn u8(&mut self, _v: &mut u8) -> Result<(), SnapshotError> {
        Ok(())
    }

    fn u32(&mut self, _v: &mut u32) -> Result<(), SnapshotError> {
        Ok(())
    }

    fn u64(&mut self, _v: &mut u64) -> Result<(), SnapshotError> {
        Ok(())
    }

    fn len(&mut self, n: usize, _what: &'static str) -> Result<usize, SnapshotError> {
        Ok(n)
    }

    fn count(&mut self, n: usize, what: &'static str) -> Result<(), SnapshotError> {
        self.label(b'c', what);
        self.hasher.update((n as u64).to_le_bytes());
        Ok(())
    }

    fn string(&mut self, _v: &mut String) -> Result<(), SnapshotError> {
        Ok(())
    }

    fn kind(&mut self, name: &'static str) {
        self.label(b'k', name);
    }
}
