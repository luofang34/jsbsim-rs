// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/atmosphere/FGMars.{h,cpp} and the
// FGAtmosphere base class it extends (constructor, InitModel).
//
//! The Martian atmosphere (`FGMars`): the NASA Glenn curve fits for
//! temperature and pressure, and density from the gas law with the mean
//! molar mass of the Martian air.

use super::{
    Atmosphere, AtmosphereModel, RSTAR, SH_RATIO, STD_DAY_SL_PRESSURE, STD_DAY_SL_TEMPERATURE,
};
use crate::units::KGTOSLUG;

/// `Mmars`: mean molecular weight of the Martian atmosphere, slug/mol.
pub const MMARS: f64 = 43.49 * KGTOSLUG / 1000.0;

impl Atmosphere {
    /// `FGMars(fdmex)`: `FGAtmosphere`'s initial state with the Martian
    /// gas constant.
    #[must_use]
    pub fn mars() -> Self {
        let standard = Self::default();
        Self {
            model: AtmosphereModel::Mars,
            sl_temperature: 1.8,
            sl_density: 1.0,
            sl_pressure: 1.0,
            sl_soundspeed: 1.0,
            temperature: 1.8,
            density: 0.0,
            pressure: 0.0,
            sound_speed: 0.0,
            pressure_altitude: 0.0,
            density_altitude: 0.0,
            viscosity: 0.0,
            kinematic_viscosity: 0.0,
            reng: RSTAR / MMARS,
            ..standard
        }
    }

    /// `FGAtmosphere::InitModel`, which, unlike the standard atmosphere's,
    /// restarts the model's rate counter.
    pub(super) fn mars_init_model(&mut self) {
        self.base.init();
        self.temperature = STD_DAY_SL_TEMPERATURE;
        self.sl_temperature = self.temperature;
        self.pressure = STD_DAY_SL_PRESSURE;
        self.sl_pressure = self.pressure;
        self.density = self.pressure / (self.reng * self.temperature);
        self.sl_density = self.density;
        self.sound_speed = self.std_day_sl_soundspeed;
        self.sl_soundspeed = self.sound_speed;
        self.mars_calculate(0.0);
    }

    /// `FGMars::Calculate`: temperature, pressure and density only; the
    /// speed of sound, viscosities and pressure and density altitudes keep
    /// the values `InitModel` left.
    pub(super) fn mars_calculate(&mut self, altitude: f64) {
        self.temperature = if altitude < 22960.0 {
            -25.68 - 0.000548 * altitude + 459.67
        } else {
            -10.34 - 0.001217 * altitude + 459.67
        };
        self.pressure = 14.62 * libm::exp(-0.00003 * altitude);
        self.density = self.pressure / (self.reng * self.temperature);
    }

    /// `FGAtmosphere::GetSoundSpeed(altitude)` with `FGMars`'s
    /// temperature, which does not depend on the altitude.
    pub(super) fn mars_sound_speed(&self) -> f64 {
        (SH_RATIO * self.reng * self.temperature).sqrt()
    }
}
