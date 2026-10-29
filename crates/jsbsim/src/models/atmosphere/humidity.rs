// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/atmosphere/FGStandardAtmosphere.{h,cpp}
// (water vapor).
//
//! Water vapor: dew point, vapor pressure, relative humidity and mass
//! fraction, and the gas constant of the humid air they give.
//!
//! The saturation pressure follows Sonntag (1990), valid from -45 °C to
//! 60 °C within ±0.35 °C.

use super::standard::RDRY;
use super::{Atmosphere, PressureUnit, RSTAR, TemperatureUnit};
use crate::c_numeric::format_g;
use crate::units::{KGTOSLUG, PSFTOPA, celsius_to_rankine, rankine_to_celsius};

/// Sonntag `a`, psf.
const A: f64 = 611.2 / PSFTOPA;
/// Sonntag `b`, 1/°C.
const B: f64 = 17.62;
/// Sonntag `c`, °C.
const C: f64 = 243.12;
/// `Mwater`: molecular weight of water, slug/mol.
const MWATER: f64 = 18.016 * KGTOSLUG / 1000.0;
/// `Rwater`: gas constant of water vapor.
const RWATER: f64 = RSTAR / MWATER;

impl Atmosphere {
    /// Partial pressure of the vapor in the current air, psf.
    fn vapor_pressure_psf(&self) -> f64 {
        self.pressure * self.vapor_mass_fraction / (self.vapor_mass_fraction + RDRY / RWATER)
    }

    /// `CalculateVaporPressure`: saturation pressure at `temperature` (°R),
    /// psf.
    #[must_use]
    pub fn calculate_vapor_pressure(temperature: f64) -> f64 {
        let temperature_deg_c = rankine_to_celsius(temperature);
        A * crate::elementary::exp(B * temperature_deg_c / (C + temperature_deg_c))
    }

    /// `ValidateVaporMassFraction`: caps the vapor at saturation and at the
    /// recorded maximum for geometric altitude `h`, then updates `reng`.
    pub fn validate_vapor_mass_fraction(&mut self, h: f64) {
        if self.saturated_vapor_pressure < self.pressure {
            let vapor_pressure = self.vapor_pressure_psf();
            if vapor_pressure > self.saturated_vapor_pressure {
                self.vapor_mass_fraction = RDRY * self.saturated_vapor_pressure
                    / (RWATER * (self.pressure - self.saturated_vapor_pressure));
            }
        }
        let geo_pot_alt = self.geopotential_altitude(h);
        let max_fraction = 1E-6 * self.profile.max_vapor_mass_fraction.value_1d(geo_pot_alt);
        if self.vapor_mass_fraction > max_fraction || self.vapor_mass_fraction < 0.0 {
            self.vapor_mass_fraction = max_fraction;
        }
        self.reng = (self.vapor_mass_fraction * RWATER + RDRY) / (1.0 + self.vapor_mass_fraction);
    }

    /// `SetDewPoint`.
    pub fn set_dew_point(&mut self, unit: TemperatureUnit, dewpoint: f64) {
        let mut dew_point_r = Self::convert_to_rankine(dewpoint, unit);
        const MIN_DEW_POINT: f64 = celsius_to_rankine(-C) + 1.0;
        if dew_point_r <= MIN_DEW_POINT {
            tracing::warn!(
                "The dew point temperature {} is lower than {} R.\nDew point is therefore capped to {}",
                format_g(dew_point_r, 6),
                format_g(MIN_DEW_POINT, 6),
                format_g(MIN_DEW_POINT, 6)
            );
            dew_point_r = MIN_DEW_POINT;
        }
        let vapor_pressure = Self::calculate_vapor_pressure(dew_point_r);
        self.set_vapor_pressure(PressureUnit::Psf, vapor_pressure);
        let finalized_dew_point = self.get_dew_point(TemperatureUnit::Rankine);
        if finalized_dew_point < dew_point_r {
            tracing::warn!(
                "Dew point temperature has been capped to {}",
                format_g(finalized_dew_point, 6)
            );
        }
    }

    /// `GetDewPoint`.
    #[must_use]
    pub fn get_dew_point(&self, to: TemperatureUnit) -> f64 {
        let vapor_pressure = self.vapor_pressure_psf();
        let dewpoint_deg_c = if vapor_pressure <= 0.0 {
            -C
        } else {
            let x = libm::log(vapor_pressure / A);
            C * x / (B - x)
        };
        Self::convert_from_rankine(celsius_to_rankine(dewpoint_deg_c), to)
    }

    /// `SetVaporPressure`.
    pub fn set_vapor_pressure(&mut self, unit: PressureUnit, pa: f64) {
        let altitude = self.calculate_pressure_altitude(self.pressure, 0.0);
        let mut vapor_pressure = Self::convert_to_psf(pa, unit);
        if vapor_pressure < 0.0 {
            tracing::warn!("The vapor pressure cannot be negative.\nVapor pressure is set to 0.0");
            vapor_pressure = 0.0;
        } else if vapor_pressure >= self.pressure {
            tracing::warn!(
                "The vapor pressure {} PSF is higher than the ambient pressure.\nVapor pressure is therefore capped to {}",
                format_g(vapor_pressure, 6),
                format_g(self.pressure - 1.0, 6)
            );
            vapor_pressure = self.pressure - 1.0;
        }
        self.vapor_mass_fraction =
            RDRY * vapor_pressure / (RWATER * (self.pressure - vapor_pressure));
        self.validate_vapor_mass_fraction(altitude);
    }

    /// `GetVaporPressure`.
    #[must_use]
    pub fn get_vapor_pressure(&self, to: PressureUnit) -> f64 {
        Self::convert_from_psf(self.vapor_pressure_psf(), to)
    }

    /// `GetSaturatedVaporPressure`.
    #[must_use]
    pub fn get_saturated_vapor_pressure(&self, to: PressureUnit) -> f64 {
        Self::convert_from_psf(self.saturated_vapor_pressure, to)
    }

    /// `GetRelativeHumidity`, percent.
    #[must_use]
    pub fn get_relative_humidity(&self) -> f64 {
        let vapor_pressure = self.vapor_pressure_psf();
        100.0 * vapor_pressure / self.saturated_vapor_pressure
    }

    /// `SetRelativeHumidity`, percent, clamped to [0, 100].
    pub fn set_relative_humidity(&mut self, rh: f64) {
        let mut rh = rh;
        if rh < 0.0 {
            tracing::warn!(
                "The relative humidity cannot be negative.\nRelative humidity is set to 0%"
            );
            rh = 0.0;
        } else if rh > 100.0 {
            tracing::warn!(
                "The relative humidity cannot be higher than 100%.\nRelative humidity is set to 100%"
            );
            rh = 100.0;
        }
        let vapor_pressure = 0.01 * rh * self.saturated_vapor_pressure;
        self.set_vapor_pressure(PressureUnit::Psf, vapor_pressure);
    }

    /// `GetVaporMassFractionPPM`.
    #[must_use]
    pub fn get_vapor_mass_fraction_ppm(&self) -> f64 {
        self.vapor_mass_fraction * 1E6
    }

    /// `SetVaporMassFractionPPM`.
    pub fn set_vapor_mass_fraction_ppm(&mut self, frac: f64) {
        let altitude = self.calculate_pressure_altitude(self.pressure, 0.0);
        self.vapor_mass_fraction = frac * 1E-6;
        self.validate_vapor_mass_fraction(altitude);
        if (self.vapor_mass_fraction * 1E6 - frac).abs() > 1E-2 {
            tracing::warn!(
                "The vapor mass fraction {} has been capped to {}PPM.",
                format_g(frac, 6),
                format_g(self.vapor_mass_fraction * 1E6, 6)
            );
        }
    }
}
