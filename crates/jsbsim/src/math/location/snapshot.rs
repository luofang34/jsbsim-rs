// SPDX-License-Identifier: LGPL-2.1-or-later
//! A location in a snapshot.

use super::{Ellipse, Location};
use crate::snapshot::{SnapshotError, Visit, Visitor};

/// The ECEF coordinates and the ellipse; the derived geodetic values are
/// dropped on restore and recomputed when next read.
impl Visit for Location {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        self.ec_loc.visit(v)?;
        let mut present = self.ellipse.is_some();
        present.visit(v)?;
        if present {
            let e = self.ellipse.get_or_insert(Ellipse::new(1.0, 1.0));
            for x in [&mut e.a, &mut e.e2, &mut e.c, &mut e.ec, &mut e.ec2] {
                x.visit(v)?;
            }
        } else {
            self.ellipse = None;
        }
        if v.loading() {
            self.derived = None;
        }
        Ok(())
    }
}
