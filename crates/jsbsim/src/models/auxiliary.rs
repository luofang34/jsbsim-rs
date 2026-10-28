// SPDX-License-Identifier: LGPL-2.1-or-later
//! Derived flight quantities (`FGAuxiliary`). Placeholder: being ported.

use super::base::ModelBase;
use crate::math::{ColumnVector3, Matrix33};

/// Derived quantities.
#[derive(Debug, Clone, Default)]
pub struct Auxiliary {
    /// Scheduling.
    pub base: ModelBase,
    /// Angle of attack, rad.
    pub alpha: f64,
    /// Sideslip, rad.
    pub beta: f64,
    /// Dynamic pressure, psf.
    pub qbar: f64,
    /// True airspeed, ft/s.
    pub vt: f64,
    /// Body to wind.
    pub tb2w: Matrix33,
    /// Wind to body.
    pub tw2b: Matrix33,
    /// Aero-relative body velocity.
    pub aero_uvw: ColumnVector3,
}
