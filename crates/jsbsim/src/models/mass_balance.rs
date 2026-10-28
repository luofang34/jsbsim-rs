// SPDX-License-Identifier: LGPL-2.1-or-later
//! Mass, centre of gravity and inertia (`FGMassBalance`). Placeholder: being
//! ported.

use super::base::ModelBase;
use crate::math::{ColumnVector3, Matrix33};

/// Mass properties.
#[derive(Debug, Clone, Default)]
pub struct MassBalance {
    /// Scheduling.
    pub base: ModelBase,
    /// Mass, slugs.
    pub mass: f64,
    /// Empty weight, lbs.
    pub empty_weight: f64,
    /// CG, structural frame, in.
    pub xyz_cg: ColumnVector3,
    /// Inertia tensor, slug·ft^2.
    pub j: Matrix33,
    /// Its inverse.
    pub jinv: Matrix33,
}

impl MassBalance {
    /// `StructuralToBody`: structural (in, x aft) to body (ft, x forward)
    /// relative to the CG.
    #[must_use]
    pub fn structural_to_body(&self, r: ColumnVector3) -> ColumnVector3 {
        let x = &self.xyz_cg;
        let k = crate::units::INCHTOFT;
        ColumnVector3::new(k * (x[1] - r[1]), k * (r[2] - x[2]), k * (x[3] - r[3]))
    }
}

impl MassBalance {
    /// `Load(<mass_balance>)`; being ported.
    pub fn load(&mut self, el: &crate::xml::Element) -> Result<(), crate::Error> {
        Err(crate::Error::Unsupported {
            construct: "<mass_balance> (FGMassBalance is being ported)".to_owned(),
            path: el.location().to_string(),
        })
    }
}
