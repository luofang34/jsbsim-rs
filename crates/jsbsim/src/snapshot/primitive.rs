// SPDX-License-Identifier: LGPL-2.1-or-later
//! [`Visit`] for numbers, flags and the containers models hold them in.

use std::cell::Cell;

use super::{SnapshotError, Visit, Visitor};
use crate::properties::NodeId;

impl Visit for u8 {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        v.u8(self)
    }
}

impl Visit for u32 {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        v.u32(self)
    }
}

impl Visit for u64 {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        v.u64(self)
    }
}

impl Visit for i32 {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        let mut bits = *self as u32;
        v.u32(&mut bits)?;
        *self = bits as i32;
        Ok(())
    }
}

impl Visit for i64 {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        let mut bits = *self as u64;
        v.u64(&mut bits)?;
        *self = bits as i64;
        Ok(())
    }
}

impl Visit for usize {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        let mut wide = *self as u64;
        v.u64(&mut wide)?;
        *self = usize::try_from(wide).map_err(|_| SnapshotError::InvalidValue {
            what: "usize",
            value: wide,
        })?;
        Ok(())
    }
}

impl Visit for f64 {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        let mut bits = self.to_bits();
        v.u64(&mut bits)?;
        *self = f64::from_bits(bits);
        Ok(())
    }
}

impl Visit for f32 {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        let mut bits = self.to_bits();
        v.u32(&mut bits)?;
        *self = f32::from_bits(bits);
        Ok(())
    }
}

impl Visit for bool {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        let mut byte = u8::from(*self);
        v.u8(&mut byte)?;
        *self = match byte {
            0 => false,
            1 => true,
            other => {
                return Err(SnapshotError::InvalidValue {
                    what: "bool",
                    value: u64::from(other),
                });
            }
        };
        Ok(())
    }
}

impl Visit for String {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        v.string(self)
    }
}

impl Visit for NodeId {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        let mut slot = self.slot() as u32;
        v.u32(&mut slot)?;
        *self = NodeId::from_u32(slot);
        Ok(())
    }
}

impl<T: Visit> Visit for Cell<T> {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        self.get_mut().visit(v)
    }
}

impl<T: Visit, const N: usize> Visit for [T; N] {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        self.iter_mut().try_for_each(|x| x.visit(v))
    }
}

/// A list whose length the loaded model fixes.
impl<T: Visit> Visit for Vec<T> {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        v.count(self.len(), "list entries")?;
        self.iter_mut().try_for_each(|x| x.visit(v))
    }
}

/// A slice whose length the loaded model fixes.
impl<T: Visit> Visit for Box<[T]> {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        v.count(self.len(), "list entries")?;
        self.iter_mut().try_for_each(|x| x.visit(v))
    }
}

impl<T: Visit> Visit for Box<T> {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        (**self).visit(v)
    }
}

impl<A: Visit, B: Visit> Visit for (A, B) {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        self.0.visit(v)?;
        self.1.visit(v)
    }
}

/// An optional value that is state: whether it is present, then the
/// value. One that appears while restoring starts from its default.
impl<T: Visit + Default> Visit for Option<T> {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        let mut present = self.is_some();
        present.visit(v)?;
        match (present, self.as_mut()) {
            (true, Some(x)) => x.visit(v),
            (true, None) => self.insert(T::default()).visit(v),
            (false, _) => {
                *self = None;
                Ok(())
            }
        }
    }
}

/// A node that may not exist yet, as `u32::MAX` when it does not.
impl Visit for Option<NodeId> {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        let mut slot = self.map_or(u32::MAX, |n| n.slot() as u32);
        v.u32(&mut slot)?;
        *self = (slot != u32::MAX).then(|| NodeId::from_u32(slot));
        Ok(())
    }
}

/// An optional part the loaded model fixes, such as a component's clip:
/// visited when present, and not recorded otherwise.
pub(crate) fn visit_present<T: Visit, V: Visitor>(
    x: &mut Option<T>,
    v: &mut V,
) -> Result<(), SnapshotError> {
    v.count(usize::from(x.is_some()), "optional parts")?;
    x.as_mut().map_or(Ok(()), |x| x.visit(v))
}

/// Optional parts the loaded model fixes, such as a table's lookup
/// properties, each visited when present.
pub(crate) fn visit_each_present<'a, T: Visit + 'a, V: Visitor>(
    parts: impl IntoIterator<Item = &'a mut Option<T>>,
    v: &mut V,
) -> Result<(), SnapshotError> {
    parts.into_iter().try_for_each(|x| visit_present(x, v))
}

/// A list whose length is state: restoring resizes it, which allocates
/// only when it must grow past the capacity it has had.
pub(crate) fn visit_dynamic<T: Visit + Default, V: Visitor>(
    list: &mut Vec<T>,
    v: &mut V,
    what: &'static str,
) -> Result<(), SnapshotError> {
    let n = v.len(list.len(), what)?;
    if v.loading() {
        list.resize_with(n, T::default);
    }
    list.iter_mut().take(n).try_for_each(|x| x.visit(v))
}
