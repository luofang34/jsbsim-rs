// SPDX-License-Identifier: LGPL-2.1-or-later
//! A location in a snapshot.

use super::{Ellipse, Location};
use crate::snapshot::{SnapshotError, Visit, Visitor, visit_flag};

/// The ECEF coordinates and the ellipse; the derived geodetic values are
/// dropped on restore and recomputed when next read.
impl Visit for Location {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        self.ec_loc.visit(v)?;
        let present = visit_flag(self.ellipse.is_some(), v)?;
        match (present, self.ellipse.as_mut()) {
            (true, Some(e)) => visit_ellipse(e, v)?,
            (true, None) if v.mutating() => {
                visit_ellipse(self.ellipse.insert(Ellipse::new(1.0, 1.0)), v)?;
            }
            // A check walks the ellipse a restore would make.
            (true, None) => visit_ellipse(&mut Ellipse::new(1.0, 1.0), v)?,
            (false, _) if v.mutating() => self.ellipse = None,
            (false, _) => {}
        }
        if v.mutating() {
            self.derived = None;
        }
        Ok(())
    }
}

fn visit_ellipse<V: Visitor>(e: &mut Ellipse, v: &mut V) -> Result<(), SnapshotError> {
    for x in [&mut e.a, &mut e.e2, &mut e.c, &mut e.ec, &mut e.ec2] {
        x.visit(v)?;
    }
    Ok(())
}
