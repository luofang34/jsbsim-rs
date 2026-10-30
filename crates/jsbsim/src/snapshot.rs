// SPDX-License-Identifier: LGPL-2.1-or-later
//! Snapshots of an [`Fdm`](crate::fdm::Fdm)'s dynamic state, for replay,
//! rewind and lockstep with another simulation. JSBSim has no equivalent.
//!
//! A snapshot holds every value a later [`Fdm::run`](crate::fdm::Fdm::run)
//! can read: each property's value, every model's state, the random
//! generators, the script's event bookkeeping, the simulation time and
//! the frame counter. It holds none of the structure the loaded aircraft
//! fixes, so it restores only onto an executive that loaded the same
//! model the same way, which its [`Fingerprint`] checks. The ground
//! callback belongs to the host and is neither saved nor replaced.
//!
//! # Format
//!
//! A snapshot is one little-endian byte stream, the same in memory and
//! on the wire, so [`Snapshot::as_bytes`] can cross to another target:
//!
//! - the header: [`MAGIC`], [`FORMAT_VERSION`] as `u32`, then the
//!   fingerprint's three SHA-256 digests (documents, load-time property
//!   tree, model structure) and the load-time node count as `u32`;
//! - the property tree: its lookup generation, the nodes created after
//!   loading (parent slot, index, name), then each node's attribute bits
//!   and value in node order, keyed by position rather than name;
//! - every model with the executive, the functions and tables, then the
//!   script's events and the trims run.
//!
//! Every `f64` is its `to_bits` as `u64`, NaN payloads included; `bool`
//! is one byte, 0 or 1; `i32` and `i64` are their two's complement as
//! `u32` and `u64`; a `usize` value is a `u64`, but a length or count is a
//! `u32`; a string is its length and UTF-8 bytes. Derived caches (a location's geodetic values,
//! a quaternion's matrices) are not stored: restoring drops them, and they
//! are recomputed with the same operations on their next read.

mod codec;
mod error;
mod fingerprint;
mod primitive;
mod shape;

#[cfg(test)]
mod tests;

pub use error::{FingerprintPart, SnapshotError};
pub use fingerprint::Fingerprint;

pub(crate) use codec::{Checker, Reader, Writer, count_u32};
pub(crate) use fingerprint::{HashingResolver, LoadRecord};
pub(crate) use primitive::{visit_dynamic, visit_each_present, visit_flag, visit_present};
pub(crate) use shape::ShapeHasher;

/// The first bytes of every snapshot.
pub const MAGIC: [u8; 8] = *b"JSBSNAP\0";

/// The version of the byte layout. Any change to what is stored, or in
/// which order, needs a new version: the golden stream of
/// `tools/oracle/tests/snapshot_golden.rs` pins this one.
pub const FORMAT_VERSION: u32 = 1;

/// Bytes before the body: magic, version, three digests, node count.
pub const HEADER_LEN: usize = 8 + 4 + 3 * 32 + 4;

/// A saved state of an [`Fdm`](crate::fdm::Fdm).
///
/// Taking a snapshot into one that already holds a snapshot of the same
/// executive reuses its buffer, so after the first, neither
/// [`Fdm::snapshot_into`](crate::fdm::Fdm::snapshot_into) nor
/// [`Fdm::restore`](crate::fdm::Fdm::restore) allocates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    bytes: Vec<u8>,
}

impl Snapshot {
    /// An empty snapshot to take one into.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The encoded snapshot, header and body.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// A snapshot from bytes [`as_bytes`](Self::as_bytes) gave, on this
    /// target or another. The header is checked here; the body when it is
    /// restored.
    ///
    /// # Errors
    ///
    /// [`SnapshotError::BadMagic`], [`SnapshotError::VersionMismatch`] or
    /// [`SnapshotError::Truncated`] when `bytes` do not start with a
    /// header this build reads.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SnapshotError> {
        Fingerprint::read_header(bytes)?;
        Ok(Self {
            bytes: bytes.to_vec(),
        })
    }

    /// The fingerprint of the model the snapshot was taken of.
    ///
    /// # Errors
    ///
    /// As [`from_bytes`](Self::from_bytes), for a snapshot built from
    /// bytes that were changed since.
    pub fn fingerprint(&self) -> Result<Fingerprint, SnapshotError> {
        Fingerprint::read_header(&self.bytes)
    }

    pub(crate) fn buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.bytes
    }
}

/// Dynamic state that a [`Visitor`] saves or restores in place.
///
/// One walk serves every direction, so what is written and what is read
/// cannot drift apart.
pub(crate) trait Visit {
    /// Saves or restores `self` through `v`.
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError>;
}

/// A direction of a [`Visit`] walk: saving, checking a snapshot before
/// it is restored, restoring, or hashing the model structure.
///
/// The `u8`, `u32` and `u64` of a decoding visitor overwrite their
/// argument, so an impl decodes into a local and assigns it to `self`
/// only when [`mutating`](Self::mutating).
pub(crate) trait Visitor {
    /// True when values come from a snapshot: optional values, list
    /// lengths and value kinds then follow the stream, not `self`.
    fn decoding(&self) -> bool;
    /// True when the walk overwrites what it visits. A decoding walk that
    /// does not mutate checks a snapshot: where the stream holds a part
    /// `self` lacks, it walks a default temporary instead of making one.
    fn mutating(&self) -> bool;
    /// One byte.
    fn u8(&mut self, v: &mut u8) -> Result<(), SnapshotError>;
    /// Four bytes.
    fn u32(&mut self, v: &mut u32) -> Result<(), SnapshotError>;
    /// Eight bytes.
    fn u64(&mut self, v: &mut u64) -> Result<(), SnapshotError>;
    /// A length the state itself may change: restoring returns the saved
    /// one for the caller to resize to.
    fn len(&mut self, n: usize, what: &'static str) -> Result<usize, SnapshotError>;
    /// A count the loaded model fixes; restoring fails on another.
    fn count(&mut self, n: usize, what: &'static str) -> Result<(), SnapshotError>;
    /// Text.
    fn string(&mut self, v: &mut String) -> Result<(), SnapshotError>;
    /// The kind of a structural variant, which only the fingerprint
    /// records: the loaded model fixes it.
    fn kind(&mut self, name: &'static str);
}

/// Implements [`Visit`] for a struct by visiting its fields in groups:
/// state fields; `parts`, optional parts the loaded model fixes;
/// `dynamic`, lists whose length is state; `each`, lists of optional
/// parts. `skip` names the fields that are structure alone. The pattern
/// is exhaustive, so a field added to the struct fails to compile until
/// it is listed.
macro_rules! visit_fields {
    ($ty:ty { $($field:ident),* $(,)? }
        $(parts { $($part:ident),* $(,)? })?
        $(dynamic { $($dynamic:ident),* $(,)? })?
        $(each { $($each:ident),* $(,)? })?
        $(skip { $($skip:ident),* $(,)? })?
    ) => {
        impl $crate::snapshot::Visit for $ty {
            fn visit<V: $crate::snapshot::Visitor>(
                &mut self,
                v: &mut V,
            ) -> Result<(), $crate::snapshot::SnapshotError> {
                let Self {
                    $($field,)*
                    $($($part,)*)?
                    $($($dynamic,)*)?
                    $($($each,)*)?
                    $($($skip: _,)*)?
                } = self;
                $($crate::snapshot::Visit::visit($field, v)?;)*
                $($($crate::snapshot::visit_present($part, v)?;)*)?
                $($($crate::snapshot::visit_dynamic($dynamic, v, stringify!($dynamic))?;)*)?
                $($($crate::snapshot::visit_each_present($each, v)?;)*)?
                let _ = v;
                Ok(())
            }
        }
    };
}
pub(crate) use visit_fields;

/// Implements [`Visit`] for a field-less enum whose value is state, as
/// its variant's position.
macro_rules! visit_unit_enum {
    ($ty:ty { $($variant:ident),+ $(,)? }) => {
        impl $crate::snapshot::Visit for $ty {
            fn visit<V: $crate::snapshot::Visitor>(
                &mut self,
                v: &mut V,
            ) -> Result<(), $crate::snapshot::SnapshotError> {
                const ALL: &[$ty] = &[$(<$ty>::$variant),+];
                let mut tag = ALL.iter().position(|x| x == self).unwrap_or(0) as u8;
                v.u8(&mut tag)?;
                match ALL.get(usize::from(tag)) {
                    Some(x) => {
                        if v.mutating() {
                            *self = *x;
                        }
                        Ok(())
                    }
                    None => Err($crate::snapshot::SnapshotError::InvalidValue {
                        what: stringify!($ty),
                        value: u64::from(tag),
                    }),
                }
            }
        }
    };
}
pub(crate) use visit_unit_enum;
