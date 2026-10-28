// SPDX-License-Identifier: LGPL-2.1-or-later
//! Winds (`FGWinds`). Placeholder: steady wind only, being ported.

use super::base::ModelBase;
use crate::math::ColumnVector3;

/// Winds.
#[derive(Debug, Clone, Default)]
pub struct Winds {
    /// Scheduling.
    pub base: ModelBase,
    /// Steady wind, NED, ft/s.
    pub wind_ned: ColumnVector3,
    /// Total wind including gusts and turbulence, NED, ft/s.
    pub total_wind_ned: ColumnVector3,
    /// Turbulence rates, body, rad/s.
    pub turb_pqr: ColumnVector3,
}
