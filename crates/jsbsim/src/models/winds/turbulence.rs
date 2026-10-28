// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/atmosphere/FGWinds.cpp (Turbulence).
//
//! The deterministic branches of `FGWinds::Turbulence`. Every branch that
//! would draw from JSBSim's random number generator, which this crate does
//! not provide, is unsupported.

use super::Winds;
use crate::Error;
use crate::math::{ColumnVector3, E_EAST, E_NORTH};
use crate::units::RADTODEG;

/// `tType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurbulenceType {
    /// `ttNone`.
    None,
    /// `ttStandard`: JSBSim does not implement it, so it is turbulence-free.
    Standard,
    /// `ttCulp`.
    Culp,
    /// `ttMilspec`: Dryden spectrum, MIL-STD-1797A discretization.
    Milspec,
    /// `ttTustin`: Dryden spectrum, Tustin discretization.
    Tustin,
}

impl TurbulenceType {
    /// The model of an `atmosphere/turb-type` value; `None` outside the
    /// enum, which JSBSim's switch treats as no turbulence.
    #[must_use]
    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::None),
            1 => Some(Self::Standard),
            2 => Some(Self::Culp),
            3 => Some(Self::Milspec),
            4 => Some(Self::Tustin),
            _ => None,
        }
    }

    /// The enum value.
    #[must_use]
    pub fn code(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Standard => 1,
            Self::Culp => 2,
            Self::Milspec => 3,
            Self::Tustin => 4,
        }
    }
}

impl Winds {
    /// `GetTurbMagnitude`, ft/s.
    #[must_use]
    pub fn turbulence_magnitude(&self) -> f64 {
        self.turbulence_ned.magnitude()
    }

    /// `FGWinds::Turbulence`. The early returns of the disabled Culp and
    /// Dryden models skip the `TurbDirection` update, as in JSBSim.
    pub(super) fn turbulence_step(&mut self) -> Result<(), Error> {
        let t = self.turbulence;
        match TurbulenceType::from_code(t.turb_type) {
            Some(TurbulenceType::Culp) => {
                // `wind_from_clockwise`, which nothing sets, seeds the roll
                // rate.
                self.turb_pqr = ColumnVector3::zero();
                if t.gain != 0.0 {
                    return Err(self.unsupported_turbulence());
                }
                self.turbulence_ned = ColumnVector3::zero();
                return Ok(());
            }
            Some(TurbulenceType::Milspec | TurbulenceType::Tustin) => {
                // Airspeed relative to the air mass without the turbulence.
                let airmass =
                    self.input.tl2b * (self.wind_ned + self.gust_ned + self.cosine_gust_ned);
                let v = (self.input.uvw - airmass).magnitude();
                if t.severity != 0 && v != 0.0 {
                    return Err(self.unsupported_turbulence());
                }
                self.turbulence_ned = ColumnVector3::zero();
                self.turb_pqr = ColumnVector3::zero();
                return Ok(());
            }
            Some(TurbulenceType::None | TurbulenceType::Standard) | None => {
                self.turbulence_ned = ColumnVector3::zero();
                self.turb_pqr = ColumnVector3::zero();
            }
        }
        let ned = self.turbulence_ned;
        self.turbulence.direction = libm::atan2(ned[E_EAST], ned[E_NORTH]) * RADTODEG;
        Ok(())
    }

    fn unsupported_turbulence(&self) -> Error {
        Error::Unsupported {
            construct: format!(
                "atmosphere/turb-type {} (FGWinds turbulence)",
                self.turbulence.turb_type
            ),
            path: "winds".to_owned(),
        }
    }
}
