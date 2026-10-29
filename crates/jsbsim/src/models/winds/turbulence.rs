// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/atmosphere/FGWinds.cpp (Turbulence).
//
//! `FGWinds::Turbulence`: the type dispatch and Culp's roll and vertical
//! model; the Dryden models are in [`dryden`].

mod dryden;

pub use dryden::{DrydenState, PoeTable};

use super::Winds;
use crate::math::{ColumnVector3, E_DOWN, E_EAST, E_NORTH, E_P};
use crate::random::RandomNumberGenerator;
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

/// Culp's state, which `InitModel` leaves alone.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CulpState {
    /// `spike`: the decaying gust pulse.
    pub spike: f64,
    /// `target_time`: when the next pulse fires; 0 draws a new one.
    pub target_time: f64,
    /// `strength`: the pulse's uniform deviate.
    pub strength: f64,
}

/// `wind_from_clockwise`, which nothing sets: Culp's roll rate offset.
const WIND_FROM_CLOCKWISE: f64 = 0.0;

/// 2 pi as JSBSim's Culp model spells it, to ten digits, which rounds to
/// another double than `TAU`.
#[allow(clippy::approx_constant)]
const CULP_TWO_PI: f64 = 6.283185307;

impl Winds {
    /// `GetTurbMagnitude`, ft/s.
    #[must_use]
    pub fn turbulence_magnitude(&self) -> f64 {
        self.turbulence_ned.magnitude()
    }

    /// `FGWinds::Turbulence` at time `time`, drawing from the winds' own
    /// generator if they have one, else from `exec_rng`. The early returns
    /// of the disabled Culp and Dryden models skip the `TurbDirection`
    /// update, as in JSBSim.
    pub(super) fn turbulence_step(&mut self, exec_rng: &RandomNumberGenerator, time: f64) {
        let generator = self.generator.take();
        let rng = generator.as_ref().unwrap_or(exec_rng);
        let updated = match TurbulenceType::from_code(self.turbulence.turb_type) {
            Some(TurbulenceType::Culp) => self.culp_step(rng, time),
            Some(TurbulenceType::Milspec) => self.dryden_step(rng, false),
            Some(TurbulenceType::Tustin) => self.dryden_step(rng, true),
            Some(TurbulenceType::None | TurbulenceType::Standard) | None => {
                self.turbulence_ned = ColumnVector3::zero();
                self.turb_pqr = ColumnVector3::zero();
                true
            }
        };
        self.generator = generator;
        if updated {
            let ned = self.turbulence_ned;
            self.turbulence.direction = libm::atan2(ned[E_EAST], ned[E_NORTH]) * RADTODEG;
        }
    }

    /// The `ttCulp` case; false when a zero gain returns early.
    fn culp_step(&mut self, rng: &RandomNumberGenerator, time: f64) -> bool {
        self.turb_pqr = ColumnVector3::new(WIND_FROM_CLOCKWISE, 0.0, 0.0);
        let t = &mut self.turbulence;
        if t.gain == 0.0 {
            self.turbulence_ned = ColumnVector3::zero();
            return false;
        }
        // The limits are written back, as JSBSim keeps them.
        t.gain = t.gain.clamp(0.0, 1.0);
        t.rate = t.rate.clamp(0.0, 30.0);
        t.rhythmicity = t.rhythmicity.clamp(0.0, 1.0);
        let sinewave = libm::sin(time * t.rate * CULP_TWO_PI);
        let c = &mut self.culp;
        if c.target_time == 0.0 {
            let random = rng.uniform();
            c.strength = random;
            c.target_time = time + 0.71 + random * 0.5;
        }
        if time > c.target_time {
            c.spike = 1.0;
            c.target_time = 0.0;
        }
        // The vertical speed, ft/s, of a gain of 1.
        let max_vs = 40.0;
        let delta = c.strength * max_vs * t.gain * (1.0 - t.rhythmicity) * c.spike;
        let mut down = sinewave * max_vs * t.gain * t.rhythmicity;
        down += delta;
        let (agl, span) = (self.input.distance_agl, self.input.wingspan);
        if agl / span < 3.0 {
            down *= agl / span * 0.3333;
        }
        self.turbulence_ned[E_DOWN] = down;
        let (sin, cos) = libm::sincos(delta * 3.0);
        self.turbulence_ned[E_NORTH] = sin;
        self.turbulence_ned[E_EAST] = cos;
        // A clockwise vortex rolls the aircraft left.
        self.turb_pqr[E_P] += delta * 0.04;
        c.spike *= 0.9;
        true
    }
}
