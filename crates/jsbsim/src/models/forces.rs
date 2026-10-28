// SPDX-License-Identifier: LGPL-2.1-or-later
//! Force-producing models not ported yet (aerodynamics, propulsion, ground
//! and external reactions, buoyancy, FCS). Each contributes no force; its
//! XML is refused with [`Error::Unsupported`](crate::Error::Unsupported)
//! unless empty.

use super::base::ModelBase;
use crate::math::ColumnVector3;

/// A model that contributes a force and a moment.
#[derive(Debug, Clone, Default)]
pub struct ForceModel {
    /// Scheduling.
    pub base: ModelBase,
    /// Force, body frame, lbs.
    pub forces: ColumnVector3,
    /// Moment about the CG, body frame, lbs·ft.
    pub moments: ColumnVector3,
}
