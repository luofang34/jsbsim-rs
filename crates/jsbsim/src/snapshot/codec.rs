// SPDX-License-Identifier: LGPL-2.1-or-later
//! The saving and restoring directions of a snapshot walk.

use super::{SnapshotError, Visitor};

/// Appends what it visits to a buffer, little-endian.
pub(crate) struct Writer<'a> {
    out: &'a mut Vec<u8>,
}

impl<'a> Writer<'a> {
    pub(crate) fn new(out: &'a mut Vec<u8>) -> Self {
        Self { out }
    }

    pub(crate) fn put(&mut self, bytes: &[u8]) {
        self.out.extend_from_slice(bytes);
    }
}

fn count_u32(n: usize, what: &'static str) -> Result<u32, SnapshotError> {
    u32::try_from(n).map_err(|_| SnapshotError::InvalidValue {
        what,
        value: n as u64,
    })
}

impl Visitor for Writer<'_> {
    fn loading(&self) -> bool {
        false
    }

    fn u8(&mut self, v: &mut u8) -> Result<(), SnapshotError> {
        self.out.push(*v);
        Ok(())
    }

    fn u32(&mut self, v: &mut u32) -> Result<(), SnapshotError> {
        self.put(&v.to_le_bytes());
        Ok(())
    }

    fn u64(&mut self, v: &mut u64) -> Result<(), SnapshotError> {
        self.put(&v.to_le_bytes());
        Ok(())
    }

    fn len(&mut self, n: usize, what: &'static str) -> Result<usize, SnapshotError> {
        self.put(&count_u32(n, what)?.to_le_bytes());
        Ok(n)
    }

    fn count(&mut self, n: usize, what: &'static str) -> Result<(), SnapshotError> {
        self.len(n, what).map(|_| ())
    }

    fn string(&mut self, v: &mut String) -> Result<(), SnapshotError> {
        self.len(v.len(), "string length")?;
        self.put(v.as_bytes());
        Ok(())
    }

    fn kind(&mut self, _name: &'static str) {}
}

/// Overwrites what it visits with values read from a snapshot.
#[derive(Clone)]
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8], at: usize) -> Self {
        Self { bytes, at }
    }

    /// The next `n` bytes.
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], SnapshotError> {
        let end = self.at.checked_add(n);
        match end.and_then(|end| self.bytes.get(self.at..end)) {
            Some(slice) => {
                self.at += n;
                Ok(slice)
            }
            None => Err(SnapshotError::Truncated {
                offset: self.at,
                needed: n,
                len: self.bytes.len(),
            }),
        }
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], SnapshotError> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    pub(crate) fn read_u32(&mut self) -> Result<u32, SnapshotError> {
        self.array().map(u32::from_le_bytes)
    }

    /// Text, borrowed from the snapshot.
    pub(crate) fn read_str(&mut self) -> Result<&'a str, SnapshotError> {
        let n = self.read_u32()? as usize;
        let offset = self.at;
        std::str::from_utf8(self.take(n)?).map_err(|_| SnapshotError::InvalidText { offset })
    }

    /// Fails unless every byte was read.
    pub(crate) fn finish(&self) -> Result<(), SnapshotError> {
        match self.bytes.len() - self.at.min(self.bytes.len()) {
            0 => Ok(()),
            extra => Err(SnapshotError::TrailingBytes { extra }),
        }
    }
}

impl Visitor for Reader<'_> {
    fn loading(&self) -> bool {
        true
    }

    fn u8(&mut self, v: &mut u8) -> Result<(), SnapshotError> {
        *v = self.array::<1>()?[0];
        Ok(())
    }

    fn u32(&mut self, v: &mut u32) -> Result<(), SnapshotError> {
        *v = self.read_u32()?;
        Ok(())
    }

    fn u64(&mut self, v: &mut u64) -> Result<(), SnapshotError> {
        *v = self.array().map(u64::from_le_bytes)?;
        Ok(())
    }

    fn len(&mut self, _n: usize, _what: &'static str) -> Result<usize, SnapshotError> {
        Ok(self.read_u32()? as usize)
    }

    fn count(&mut self, n: usize, what: &'static str) -> Result<(), SnapshotError> {
        let found = self.read_u32()?;
        if found as usize == n {
            Ok(())
        } else {
            Err(SnapshotError::Shape {
                what,
                expected: n as u64,
                found: u64::from(found),
            })
        }
    }

    fn string(&mut self, v: &mut String) -> Result<(), SnapshotError> {
        let text = self.read_str()?;
        if v != text {
            v.clear();
            v.push_str(text);
        }
        Ok(())
    }

    fn kind(&mut self, _name: &'static str) {}
}
