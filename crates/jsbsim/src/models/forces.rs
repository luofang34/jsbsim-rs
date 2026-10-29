// SPDX-License-Identifier: LGPL-2.1-or-later
//! The state of a model slot with no model of its own (`FGInput`, whose
//! sockets the core does not open): scheduling, and a force and moment
//! that stay zero.

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
