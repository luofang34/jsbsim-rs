// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGAtmosphere.{h,cpp} and
// src/models/atmosphere/FGStandardAtmosphere.{h,cpp}.
//
//! The atmosphere: `FGAtmosphere`'s common state and unit handling, with
//! the 1976 U.S. Standard Atmosphere (`FGStandardAtmosphere`) the
//! executive constructs, and the Martian (`FGMars`) and NRLMSISE-00
//! (`FGMSIS`) atmospheres a planet may replace it with.
//!
//! JSBSim splits these into a base class and derived classes; here they
//! share one type, whose [`AtmosphereModel`] selects the overrides of the
//! virtual methods. The base-class versions that no model reaches are not
//! ported.

mod bind;
mod humidity;
mod mars;
mod msis;
pub mod nrlmsise;
mod override_nodes;
mod pressure;
mod snapshot;
mod standard;
#[cfg(test)]
mod tests;

pub use bind::{STANDARD_PROPERTIES, bind};
pub use mars::MMARS;
pub use msis::Msis;
pub use override_nodes::OverrideNodes;

use super::base::ModelBase;
use crate::c_numeric::format_g;
use crate::functions::Table;
use crate::units::{FTTOM, KGTOSLUG, kelvin_to_rankine};

/// `StdDaySLtemperature`, °R.
pub const STD_DAY_SL_TEMPERATURE: f64 = 518.67;
/// `StdDaySLpressure`, psf.
pub const STD_DAY_SL_PRESSURE: f64 = 2116.228;
/// `SHRatio`: ratio of specific heats of air.
pub const SH_RATIO: f64 = 1.4;
/// `Rstar`: universal gas constant, ft·lbf/R/mol.
pub const RSTAR: f64 = 8.31432 * KGTOSLUG / kelvin_to_rankine(FTTOM * FTTOM);
/// `Mair`: mean molecular weight of air, slug/mol.
pub const MAIR: f64 = 28.9645 * KGTOSLUG / 1000.0;
/// `g0`: the ISA's sea-level gravity, ft/s^2, whatever gravity model runs.
pub const G0: f64 = 9.80665 / FTTOM;
/// `Reng0`: specific gas constant of dry air, ft·lbf/slug/R.
pub const RENG0: f64 = RSTAR / MAIR;
/// `StdDaySLdensity`, slug/ft^3.
pub const STD_DAY_SL_DENSITY: f64 = STD_DAY_SL_PRESSURE / (RENG0 * STD_DAY_SL_TEMPERATURE);
/// `SutherlandConstant`, °R.
pub(crate) const SUTHERLAND_CONSTANT: f64 = 198.72;
/// `Beta`, slug/(s·ft·R^0.5).
pub(crate) const BETA: f64 = 2.269690E-08;

/// `eTemperature`. JSBSim's `eNoTempUnit` only exists to make the
/// conversions throw, so it has no counterpart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemperatureUnit {
    /// `eFahrenheit`.
    Fahrenheit,
    /// `eCelsius`.
    Celsius,
    /// `eRankine`.
    Rankine,
    /// `eKelvin`.
    Kelvin,
}

/// `ePressure`, without the throwing `eNoPressUnit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureUnit {
    /// `ePSF`.
    Psf,
    /// `eMillibars`.
    Millibars,
    /// `ePascals`.
    Pascals,
    /// `eInchesHg`.
    InchesHg,
}

/// Inputs loaded by the executive.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inputs {
    /// Altitude above sea level, ft.
    pub altitude_asl: f64,
    /// Geodetic latitude, deg.
    pub geod_latitude_deg: f64,
    /// Longitude, deg.
    pub longitude_deg: f64,
}

/// Values of the `atmosphere/override/{temperature,pressure,density}`
/// properties that exist when `Calculate` runs; each replaces the modeled
/// value.
#[derive(Debug, Clone, Copy, Default)]
pub struct Overrides {
    /// Temperature, °R.
    pub temperature: Option<f64>,
    /// Pressure, psf.
    pub pressure: Option<f64>,
    /// Density, slug/ft^3.
    pub density: Option<f64>,
}

/// The temperature profile and the pressure and density breakpoints
/// derived from it.
#[derive(Debug, Clone)]
pub struct StandardProfile {
    /// `StdSLtemperature`, °R.
    pub std_sl_temperature: f64,
    /// `StdSLdensity`, slug/ft^3.
    pub std_sl_density: f64,
    /// `StdSLpressure`, psf.
    pub std_sl_pressure: f64,
    /// `StdSLsoundspeed`, ft/s.
    pub std_sl_soundspeed: f64,
    /// `TemperatureBias`, °R.
    pub temperature_bias: f64,
    /// `TemperatureDeltaGradient`, °R/ft.
    pub temperature_delta_gradient: f64,
    /// `GradientFadeoutAltitude`, geopotential ft.
    pub gradient_fadeout_altitude: f64,
    /// `StdAtmosTemperatureTable`: °R by geopotential altitude, ft.
    pub temperature_table: Table,
    /// `MaxVaporMassFraction`: ppm by geopotential altitude, ft.
    pub max_vapor_mass_fraction: Table,
    /// `LapseRates`, °R/ft, including the graded delta.
    pub lapse_rates: Vec<f64>,
    /// `PressureBreakpoints`, psf.
    pub pressure_breakpoints: Vec<f64>,
    /// `StdPressureBreakpoints`, psf.
    pub std_pressure_breakpoints: Vec<f64>,
    /// `StdDensityBreakpoints`, slug/ft^3.
    pub std_density_breakpoints: Vec<f64>,
    /// `StdLapseRates`, °R/ft.
    pub std_lapse_rates: Vec<f64>,
}

/// Which atmosphere model the executive runs: the standard one it
/// constructs, or the one a planet's `<atmosphere model="...">` replaces it
/// with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AtmosphereModel {
    /// `FGStandardAtmosphere`.
    #[default]
    Standard,
    /// `FGMars`.
    Mars,
    /// `FGMSIS`.
    Msis,
}

impl AtmosphereModel {
    /// Every model, each once.
    pub const ALL: [Self; 3] = [Self::Standard, Self::Mars, Self::Msis];
}

/// The atmosphere.
#[derive(Debug, Clone)]
pub struct Atmosphere {
    /// The model whose overrides of `FGAtmosphere`'s virtual methods apply.
    pub model: AtmosphereModel,
    /// Scheduling.
    pub base: ModelBase,
    /// Inputs of the current frame.
    pub input: Inputs,
    /// `SLtemperature`, °R.
    pub sl_temperature: f64,
    /// `SLdensity`, slug/ft^3.
    pub sl_density: f64,
    /// `SLpressure`, psf.
    pub sl_pressure: f64,
    /// `SLsoundspeed`, ft/s.
    pub sl_soundspeed: f64,
    /// Temperature, °R.
    pub temperature: f64,
    /// Density, slug/ft^3.
    pub density: f64,
    /// Pressure, psf.
    pub pressure: f64,
    /// Speed of sound, ft/s.
    pub sound_speed: f64,
    /// `PressureAltitude`, ft.
    pub pressure_altitude: f64,
    /// `DensityAltitude`, ft.
    pub density_altitude: f64,
    /// Absolute viscosity, slug/(ft·s).
    pub viscosity: f64,
    /// Kinematic viscosity, ft^2/s.
    pub kinematic_viscosity: f64,
    /// Standard-day sea-level speed of sound, ft/s.
    pub std_day_sl_soundspeed: f64,
    /// `Reng`: gas constant of the (possibly humid) air, ft·lbf/slug/R.
    pub reng: f64,
    /// `VaporMassFraction`: water vapor mass per dry air mass.
    pub vapor_mass_fraction: f64,
    /// `SaturatedVaporPressure`, psf.
    pub saturated_vapor_pressure: f64,
    /// The standard temperature profile and its offsets.
    pub profile: StandardProfile,
    /// The `atmosphere/override/*` nodes found so far.
    pub override_nodes: OverrideNodes,
    /// What `simulation/models/FGAtmosphere/enabled` holds once a planet
    /// has replaced the atmosphere: the flag stays tied to the replaced
    /// model, so it no longer enables or disables this one.
    pub orphaned_enabled: Option<bool>,
    /// `FGMSIS`'s date and activity, used by [`AtmosphereModel::Msis`].
    pub msis: Msis,
    /// The simulation time, s, which `FGMSIS` reads for the time of day.
    pub sim_time: f64,
}

impl Default for Atmosphere {
    /// `FGStandardAtmosphere(FGFDMExec*)`.
    fn default() -> Self {
        let mut atmosphere = Self {
            model: AtmosphereModel::Standard,
            base: ModelBase::default(),
            input: Inputs::default(),
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
            std_day_sl_soundspeed: (SH_RATIO * RENG0 * STD_DAY_SL_TEMPERATURE).sqrt(),
            reng: RENG0,
            vapor_mass_fraction: 0.0,
            saturated_vapor_pressure: STD_DAY_SL_PRESSURE,
            profile: StandardProfile::new(),
            override_nodes: OverrideNodes::default(),
            orphaned_enabled: None,
            msis: Msis::default(),
            sim_time: 0.0,
        };
        atmosphere.construct_profile();
        atmosphere
    }
}

impl Atmosphere {
    /// `FGStandardAtmosphere::InitModel`. Unlike other models it does not
    /// chain to `FGModel::InitModel`, so the rate counter is left alone.
    pub fn init_model(&mut self, overrides: &Overrides) {
        if self.model == AtmosphereModel::Mars {
            self.mars_init_model();
            return;
        }
        let p = &mut self.profile;
        p.gradient_fadeout_altitude = p
            .temperature_table
            .element(p.temperature_table.num_rows(), 0);
        p.temperature_delta_gradient = 0.0;
        p.temperature_bias = 0.0;
        p.lapse_rates.clone_from(&p.std_lapse_rates);
        p.pressure_breakpoints
            .clone_from(&p.std_pressure_breakpoints);
        self.sl_pressure = p.std_sl_pressure;
        self.sl_temperature = p.std_sl_temperature;
        self.sl_density = p.std_sl_density;
        self.sl_soundspeed = p.std_sl_soundspeed;
        self.calculate(0.0, overrides);
        // `FGMSIS::InitModel` calculates once more after the standard
        // atmosphere's.
        if self.model == AtmosphereModel::Msis {
            self.msis_calculate(0.0);
        }
    }

    /// `Calculate`: the atmosphere at `altitude` (ft ASL), then, for the
    /// standard atmosphere, the humidity limits there.
    pub fn calculate(&mut self, altitude: f64, overrides: &Overrides) {
        match self.model {
            AtmosphereModel::Mars => return self.mars_calculate(altitude),
            AtmosphereModel::Msis => return self.msis_calculate(altitude),
            AtmosphereModel::Standard => {}
        }
        self.calculate_base(altitude, overrides);
        self.saturated_vapor_pressure = Self::calculate_vapor_pressure(self.temperature);
        self.validate_vapor_mass_fraction(altitude);
    }

    /// `FGAtmosphere::Calculate`.
    fn calculate_base(&mut self, altitude: f64, overrides: &Overrides) {
        let t = match overrides.temperature {
            Some(t) => t,
            None => self.get_temperature(altitude),
        };
        let p = match overrides.pressure {
            Some(p) => p,
            None => self.get_pressure(altitude),
        };
        self.temperature = Self::validate_temperature(t, "", true);
        self.pressure = Self::validate_pressure(p, "", true);
        self.density = match overrides.density {
            Some(d) => d,
            None => self.pressure / (self.reng * self.temperature),
        };
        self.sound_speed = (SH_RATIO * self.reng * self.temperature).sqrt();
        self.pressure_altitude = self.calculate_pressure_altitude(self.pressure, altitude);
        self.density_altitude = self.calculate_density_altitude(self.density, altitude);
        self.viscosity = BETA * crate::elementary::pow(self.temperature, 1.5)
            / (SUTHERLAND_CONSTANT + self.temperature);
        self.kinematic_viscosity = self.viscosity / self.density;
    }

    /// `ValidatePressure`: caps `p` at the interstellar pressure, 1e-15 Pa.
    #[must_use]
    pub fn validate_pressure(p: f64, msg: &str, quiet: bool) -> f64 {
        let min_pressure = Self::convert_to_psf(1E-15, PressureUnit::Pascals);
        if p < min_pressure {
            if !quiet {
                tracing::warn!(
                    "{msg} {} is too low.\n{msg} will be capped to {}",
                    format_g(p, 6),
                    format_g(min_pressure, 6)
                );
            }
            return min_pressure;
        }
        p
    }

    /// `ValidateTemperature`: caps `t` at 1 K, the coldest natural place
    /// known.
    #[must_use]
    pub fn validate_temperature(t: f64, msg: &str, quiet: bool) -> f64 {
        const MIN_UNIVERSE_TEMPERATURE: f64 = kelvin_to_rankine(1.0);
        if t < MIN_UNIVERSE_TEMPERATURE {
            if !quiet {
                tracing::warn!(
                    "{msg} {} is too low.\n{msg} will be capped to {}",
                    format_g(t, 6),
                    format_g(MIN_UNIVERSE_TEMPERATURE, 6)
                );
            }
            return MIN_UNIVERSE_TEMPERATURE;
        }
        t
    }

    /// `ConvertToRankine`.
    #[must_use]
    pub fn convert_to_rankine(t: f64, unit: TemperatureUnit) -> f64 {
        match unit {
            TemperatureUnit::Fahrenheit => t + 459.67,
            TemperatureUnit::Celsius => (t + 273.15) * 1.8,
            TemperatureUnit::Rankine => t,
            TemperatureUnit::Kelvin => t * 1.8,
        }
    }

    /// `ConvertFromRankine`.
    #[must_use]
    pub fn convert_from_rankine(t: f64, unit: TemperatureUnit) -> f64 {
        match unit {
            TemperatureUnit::Fahrenheit => t - 459.67,
            TemperatureUnit::Celsius => t / 1.8 - 273.15,
            TemperatureUnit::Rankine => t,
            TemperatureUnit::Kelvin => t / 1.8,
        }
    }

    /// `ConvertToPSF`.
    #[must_use]
    pub fn convert_to_psf(p: f64, unit: PressureUnit) -> f64 {
        match unit {
            PressureUnit::Psf => p,
            PressureUnit::Millibars => p * 2.08854342,
            PressureUnit::Pascals => p * 0.0208854342,
            PressureUnit::InchesHg => p * 70.7180803,
        }
    }

    /// `ConvertFromPSF`.
    #[must_use]
    pub fn convert_from_psf(p: f64, unit: PressureUnit) -> f64 {
        match unit {
            PressureUnit::Psf => p,
            PressureUnit::Millibars => p / 2.08854342,
            PressureUnit::Pascals => p / 0.0208854342,
            PressureUnit::InchesHg => p / 70.7180803,
        }
    }

    /// `GetTemperatureSL`, °R.
    #[must_use]
    pub fn get_temperature_sl(&self) -> f64 {
        self.sl_temperature
    }

    /// `GetTemperatureRatio()`: current over sea-level temperature.
    #[must_use]
    pub fn get_temperature_ratio(&self) -> f64 {
        self.temperature / self.sl_temperature
    }

    /// `GetTemperatureRatio(h)`: modeled temperature at `h` over sea level.
    #[must_use]
    pub fn get_temperature_ratio_at(&self, h: f64) -> f64 {
        self.get_temperature(h) / self.sl_temperature
    }

    /// `GetPressureSL(to)`.
    #[must_use]
    pub fn get_pressure_sl(&self, to: PressureUnit) -> f64 {
        Self::convert_from_psf(self.sl_pressure, to)
    }

    /// `GetPressureRatio`.
    #[must_use]
    pub fn get_pressure_ratio(&self) -> f64 {
        self.pressure / self.sl_pressure
    }

    /// `GetDensity(altitude)`, slug/ft^3, with the current gas constant.
    #[must_use]
    pub fn get_density(&self, altitude: f64) -> f64 {
        match self.model {
            AtmosphereModel::Mars => return self.pressure / (self.reng * self.temperature),
            AtmosphereModel::Msis => return self.msis_compute(altitude).density,
            AtmosphereModel::Standard => {}
        }
        self.get_pressure(altitude) / (self.reng * self.get_temperature(altitude))
    }

    /// `GetDensitySL`.
    #[must_use]
    pub fn get_density_sl(&self) -> f64 {
        self.sl_density
    }

    /// `GetDensityRatio`.
    #[must_use]
    pub fn get_density_ratio(&self) -> f64 {
        self.density / self.sl_density
    }

    /// `GetSoundSpeed(altitude)`, ft/s.
    #[must_use]
    pub fn get_sound_speed(&self, altitude: f64) -> f64 {
        match self.model {
            AtmosphereModel::Mars => return self.mars_sound_speed(),
            AtmosphereModel::Msis => return self.msis_sound_speed(altitude),
            AtmosphereModel::Standard => {}
        }
        (SH_RATIO * self.reng * self.get_temperature(altitude)).sqrt()
    }

    /// `GetSoundSpeedSL`.
    #[must_use]
    pub fn get_sound_speed_sl(&self) -> f64 {
        self.sl_soundspeed
    }

    /// `GetSoundSpeedRatio`.
    #[must_use]
    pub fn get_sound_speed_ratio(&self) -> f64 {
        self.sound_speed / self.sl_soundspeed
    }

    /// `GetAbsoluteViscosity`.
    #[must_use]
    pub fn get_absolute_viscosity(&self) -> f64 {
        self.viscosity
    }

    /// `GetKinematicViscosity`.
    #[must_use]
    pub fn get_kinematic_viscosity(&self) -> f64 {
        self.kinematic_viscosity
    }

    /// `GetDensityAltitude`, ft.
    #[must_use]
    pub fn get_density_altitude(&self) -> f64 {
        self.density_altitude
    }

    /// `GetPressureAltitude`, ft.
    #[must_use]
    pub fn get_pressure_altitude(&self) -> f64 {
        self.pressure_altitude
    }
}
