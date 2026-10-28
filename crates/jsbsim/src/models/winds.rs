// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/atmosphere/FGWinds.{h,cpp}.
//
//! Winds (`FGWinds`): the steady wind, the user gust, the one-minus-cosine
//! gust and the turbulence that the executive sums into the total wind.
//!
//! The stochastic turbulence models need JSBSim's random number generator,
//! which this crate does not provide: [`Winds::run`] reports them as
//! [`Error::Unsupported`] whenever C++ would draw a random number, and runs
//! every deterministic branch (turbulence off, zero severity, zero gain,
//! zero airspeed) exactly.

mod bind;
mod gust;
#[cfg(test)]
mod tests;
mod turbulence;

pub use bind::bind;
pub use gust::{CosineGust, GustFrame, GustProfile};
pub use turbulence::TurbulenceType;

use std::f64::consts::PI;

use super::base::ModelBase;
use crate::Error;
use crate::math::{ColumnVector3, E_DOWN, E_EAST, E_NORTH, E_X, E_Y, Matrix33};

/// Inputs loaded by the executive (`FGWinds::Inputs`). JSBSim's latitude,
/// longitude and planet radius serve only the up/downburst distance, which
/// nothing calls, and the executive never loads them, so they are omitted.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inputs {
    /// Body velocity, ft/s.
    pub uvw: ColumnVector3,
    /// Wing span, ft (`LoadModelConstants`).
    pub wingspan: f64,
    /// Distance above ground, ft.
    pub distance_agl: f64,
    /// Altitude above sea level, ft.
    pub altitude_asl: f64,
    /// Local to body.
    pub tl2b: Matrix33,
    /// Wind to body.
    pub tw2b: Matrix33,
    /// Time step times the model's rate, s.
    pub total_delta_t: f64,
}

/// Turbulence parameters set through properties.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurbulenceSettings {
    /// `atmosphere/turb-type`, stored as written (any `int`).
    pub turb_type: i32,
    /// `TurbGain` (Culp).
    pub gain: f64,
    /// `TurbRate`, Hz (Culp).
    pub rate: f64,
    /// `Rhythmicity` (Culp).
    pub rhythmicity: f64,
    /// `windspeed_at_20ft`, ft/s (Milspec, Tustin).
    pub windspeed_at_20ft: f64,
    /// `probability_of_exceedence_index` (`.../milspec/severity`).
    pub severity: i32,
    /// `TurbDirection`, deg: heading of the last computed turbulence.
    pub direction: f64,
}

impl Default for TurbulenceSettings {
    /// The constructor's values: Milspec, disabled by a zero severity.
    fn default() -> Self {
        Self {
            turb_type: TurbulenceType::Milspec.code(),
            gain: 1.0,
            rate: 10.0,
            rhythmicity: 0.1,
            windspeed_at_20ft: 0.0,
            severity: 0,
            direction: 0.0,
        }
    }
}

/// Winds.
#[derive(Debug, Clone, Default)]
pub struct Winds {
    /// Scheduling.
    pub base: ModelBase,
    /// Inputs of the current frame.
    pub input: Inputs,
    /// Steady wind, NED, ft/s.
    pub wind_ned: ColumnVector3,
    /// Total wind including gusts and turbulence, NED, ft/s.
    pub total_wind_ned: ColumnVector3,
    /// Turbulence rates, body, rad/s.
    pub turb_pqr: ColumnVector3,
    /// `psiw`: direction the wind blows towards, rad.
    pub psiw: f64,
    /// User gust (`vGustNED`), ft/s.
    pub gust_ned: ColumnVector3,
    /// Turbulence velocity (`vTurbulenceNED`), ft/s.
    pub turbulence_ned: ColumnVector3,
    /// One-minus-cosine gust velocity (`vCosineGust`), NED, ft/s.
    pub cosine_gust_ned: ColumnVector3,
    /// The one-minus-cosine gust definition and its progress.
    pub cosine_gust: CosineGust,
    /// Turbulence parameters.
    pub turbulence: TurbulenceSettings,
    /// Up/downburst cells requested (`NumberOfUpDownburstCells`); their
    /// effect is not modeled by JSBSim either.
    pub updownburst_cells: usize,
    /// `atmosphere/randomseed` once written; the executive's seed until
    /// then.
    pub random_seed: Option<i32>,
}

impl Winds {
    /// `FGWinds::InitModel`. The steady wind, the gust definition and the
    /// gust's transformed direction survive, as in JSBSim.
    pub fn init_model(&mut self) {
        self.base.init();
        self.psiw = 0.0;
        self.gust_ned = ColumnVector3::zero();
        self.turbulence_ned = ColumnVector3::zero();
        self.turb_pqr = ColumnVector3::zero();
        self.cosine_gust_ned = ColumnVector3::zero();
        self.cosine_gust.profile.running = false;
        self.cosine_gust.profile.elapsed_time = 0.0;
    }

    /// `FGWinds::Run`. Fails with [`Error::Unsupported`] when a turbulence
    /// model would need random numbers.
    pub fn run(&mut self, holding: bool) -> Result<(), Error> {
        if self.base.skip() || holding {
            return Ok(());
        }
        self.turbulence_step()?;
        if self.cosine_gust.profile.running {
            self.cosine_gust_step();
        }
        self.total_wind_ned =
            self.wind_ned + self.gust_ned + self.cosine_gust_ned + self.turbulence_ned;
        if self.wind_ned[E_X] != 0.0 {
            self.psiw = libm::atan2(self.wind_ned[E_Y], self.wind_ned[E_X]);
        }
        if self.psiw < 0.0 {
            self.psiw += 2.0 * PI;
        }
        Ok(())
    }

    /// `SetWindNED(const FGColumnVector3&)`, which leaves `psiw` alone.
    pub fn set_wind_ned(&mut self, wind: ColumnVector3) {
        self.wind_ned = wind;
    }

    /// `SetWindNED(idx, wind)`, which leaves `psiw` alone.
    pub fn set_wind_ned_component(&mut self, idx: usize, wind: f64) {
        self.wind_ned[idx] = wind;
    }

    /// `GetWindspeed`, ft/s.
    #[must_use]
    pub fn windspeed(&self) -> f64 {
        self.wind_ned.magnitude()
    }

    /// `SetWindspeed`: from calm, a north wind with `psiw` reset; otherwise
    /// a horizontal wind towards `psiw`, which drops the down component.
    pub fn set_windspeed(&mut self, speed: f64) {
        if self.wind_ned.magnitude() == 0.0 {
            self.psiw = 0.0;
            self.wind_ned[E_NORTH] = speed;
        } else {
            self.wind_ned[E_NORTH] = speed * libm::cos(self.psiw);
            self.wind_ned[E_EAST] = speed * libm::sin(self.psiw);
            self.wind_ned[E_DOWN] = 0.0;
        }
    }

    /// `SetWindPsi`: keeps the magnitude, so a calm wind stays a calm north
    /// "wind" with `psiw` reset to zero.
    pub fn set_wind_psi(&mut self, dir: f64) {
        let mag = self.windspeed();
        self.psiw = dir;
        self.set_windspeed(mag);
    }

    /// `GetRandomSeed`: the winds' own seed once set, else the executive's.
    #[must_use]
    pub fn random_seed(&self, exec_seed: i32) -> i32 {
        self.random_seed.unwrap_or(exec_seed)
    }

    /// `NumberOfUpDownburstCells`: a negative count clears the cells.
    pub fn set_updownburst_cells(&mut self, num: i32) {
        self.updownburst_cells = usize::try_from(num).unwrap_or(0);
    }
}
