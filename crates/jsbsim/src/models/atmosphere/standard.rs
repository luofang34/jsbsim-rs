// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/atmosphere/FGStandardAtmosphere.{h,cpp}
// (constructor, temperature profile and its modifiers).
//
//! The 1976 standard temperature profile, and the bias and graded delta
//! that shift it.

use super::{Atmosphere, Overrides, RENG0, SH_RATIO, StandardProfile, TemperatureUnit};
use crate::c_numeric::format_g;
use crate::functions::Table;
use crate::units::{FTTOM, kelvin_to_rankine};

/// `EarthRadius`: the ISA's radius for geopotential altitude, ft.
pub const EARTH_RADIUS: f64 = 6356766.0 / FTTOM;
/// `Rdry`: gas constant of dry air.
pub const RDRY: f64 = RENG0;

/// `StdAtmosTemperatureTable`: °R by geopotential altitude (ft). The last
/// row is the first layer of the high-altitude regime (91 km geometric).
const TEMPERATURE_ROWS: [(f64, f64); 9] = [
    (0.0000, 518.67),
    (36089.2388, 389.97),
    (65616.7979, 389.97),
    (104986.8766, 411.57),
    (154199.4751, 487.17),
    (167322.8346, 487.17),
    (232939.6325, 386.37),
    (278385.8268, 336.5028),
    (298556.4304, 336.5028),
];

/// `MaxVaporMassFraction`: ppm of dry air by geopotential altitude (ft);
/// record highs below 8 km, 1% highs above.
const MAX_VAPOR_ROWS: [(f64, f64); 10] = [
    (0.0000, 35000.),
    (3280.8399, 31000.),
    (6561.6798, 28000.),
    (13123.3596, 22000.),
    (19685.0394, 8900.),
    (26246.7192, 4700.),
    (32808.3990, 1300.),
    (39370.0787, 230.),
    (45931.7585, 48.),
    (52493.4383, 38.),
];

impl StandardProfile {
    /// The tables, with everything derived from them still zero.
    pub(super) fn new() -> Self {
        Self {
            std_sl_temperature: 0.0,
            std_sl_density: 0.0,
            std_sl_pressure: super::STD_DAY_SL_PRESSURE,
            std_sl_soundspeed: 0.0,
            temperature_bias: 0.0,
            temperature_delta_gradient: 0.0,
            gradient_fadeout_altitude: 0.0,
            temperature_table: Table::one_d(&TEMPERATURE_ROWS),
            max_vapor_mass_fraction: Table::one_d(&MAX_VAPOR_ROWS),
            lapse_rates: Vec::new(),
            pressure_breakpoints: Vec::new(),
            std_pressure_breakpoints: Vec::new(),
            std_density_breakpoints: Vec::new(),
            std_lapse_rates: Vec::new(),
        }
    }
}

impl Atmosphere {
    /// The body of the `FGStandardAtmosphere` constructor.
    pub(super) fn construct_profile(&mut self) {
        let num_rows = self.profile.temperature_table.num_rows();
        self.calculate_lapse_rates();
        let p = &mut self.profile;
        p.std_lapse_rates.clone_from(&p.lapse_rates);
        // Above the table's top the gradient no longer applies.
        p.gradient_fadeout_altitude = p.temperature_table.element(num_rows, 0);
        p.pressure_breakpoints.resize(num_rows, 0.0);
        let std_sl_pressure = p.std_sl_pressure;
        self.calculate_pressure_breakpoints(std_sl_pressure);
        let p = &mut self.profile;
        p.std_pressure_breakpoints
            .clone_from(&p.pressure_breakpoints);
        p.std_sl_temperature = p.temperature_table.element(1, 1);
        p.std_sl_density = p.std_sl_pressure / (RDRY * p.std_sl_temperature);
        self.calculate_std_density_breakpoints();
        let p = &mut self.profile;
        p.std_sl_soundspeed = (SH_RATIO * RDRY * p.std_sl_temperature).sqrt();
    }

    /// `GeopotentialAltitude`.
    #[must_use]
    pub fn geopotential_altitude(&self, geometalt: f64) -> f64 {
        (geometalt * EARTH_RADIUS) / (EARTH_RADIUS + geometalt)
    }

    /// `GeometricAltitude`.
    #[must_use]
    pub fn geometric_altitude(&self, geopotalt: f64) -> f64 {
        (geopotalt * EARTH_RADIUS) / (EARTH_RADIUS - geopotalt)
    }

    /// `GetTemperature(altitude)`: modeled temperature at `altitude` (ft
    /// ASL), °R, with the bias and graded delta.
    #[must_use]
    pub fn get_temperature(&self, altitude: f64) -> f64 {
        let p = &self.profile;
        let geo_pot_alt = self.geopotential_altitude(altitude);
        let mut t;
        if geo_pot_alt >= 0.0 {
            t = p.temperature_table.value_1d(geo_pot_alt);
            if geo_pot_alt <= p.gradient_fadeout_altitude {
                t -= p.temperature_delta_gradient * geo_pot_alt;
            }
        } else {
            // The first lapse rate already includes the gradient.
            t = p.temperature_table.value_1d(0.0) + geo_pot_alt * p.lapse_rates[0];
        }
        t += p.temperature_bias;
        if geo_pot_alt <= p.gradient_fadeout_altitude {
            t += p.temperature_delta_gradient * p.gradient_fadeout_altitude;
        }
        t
    }

    /// `GetStdTemperature(altitude)`, °R. Below sea level it extrapolates
    /// with the current (graded) first lapse rate, as JSBSim does.
    #[must_use]
    pub fn get_std_temperature(&self, altitude: f64) -> f64 {
        let p = &self.profile;
        let geo_pot_alt = self.geopotential_altitude(altitude);
        if geo_pot_alt >= 0.0 {
            p.temperature_table.value_1d(geo_pot_alt)
        } else {
            p.temperature_table.value_1d(0.0) + geo_pot_alt * p.lapse_rates[0]
        }
    }

    /// `GetStdTemperatureSL`.
    #[must_use]
    pub fn get_std_temperature_sl(&self) -> f64 {
        self.profile.std_sl_temperature
    }

    /// `GetStdTemperatureRatio(h)`.
    #[must_use]
    pub fn get_std_temperature_ratio(&self, h: f64) -> f64 {
        self.get_std_temperature(h) / self.profile.std_sl_temperature
    }

    /// `GetTemperatureBias(to)`: a temperature difference, so Celsius and
    /// Kelvin share a scale.
    #[must_use]
    pub fn get_temperature_bias(&self, to: TemperatureUnit) -> f64 {
        let bias = self.profile.temperature_bias;
        if matches!(to, TemperatureUnit::Celsius | TemperatureUnit::Kelvin) {
            bias / 1.80
        } else {
            bias
        }
    }

    /// `GetTemperatureDeltaGradient(to)`, per ft.
    #[must_use]
    pub fn get_temperature_delta_gradient(&self, to: TemperatureUnit) -> f64 {
        let gradient = self.profile.temperature_delta_gradient;
        if matches!(to, TemperatureUnit::Celsius | TemperatureUnit::Kelvin) {
            gradient / 1.80
        } else {
            gradient
        }
    }

    /// `SetTemperatureSL`: biases the profile so sea level is at `t`.
    pub fn set_temperature_sl(&mut self, t: f64, unit: TemperatureUnit) {
        self.set_temperature(t, 0.0, unit);
    }

    /// `SetTemperature`: biases the profile so altitude `h` (ft ASL) is at
    /// `t`.
    pub fn set_temperature(&mut self, t: f64, h: f64, unit: TemperatureUnit) {
        let target_temp = Self::convert_to_rankine(t, unit);
        let geo_pot_alt = self.geopotential_altitude(h);
        let mut bias = target_temp - self.get_std_temperature(h);
        let p = &self.profile;
        if geo_pot_alt <= p.gradient_fadeout_altitude {
            bias -= p.temperature_delta_gradient * (p.gradient_fadeout_altitude - geo_pot_alt);
        }
        self.set_temperature_bias(TemperatureUnit::Rankine, bias);
        self.calculate_pressure_breakpoints(self.sl_pressure);
        self.sl_temperature = self.get_temperature(0.0);
        self.calculate_sl_sound_speed_and_density();
    }

    /// `SetTemperatureBias`: shifts the whole profile by `t`, capped so no
    /// temperature drops below 1 K.
    pub fn set_temperature_bias(&mut self, unit: TemperatureUnit, t: f64) {
        let min_std_atmosphere_temp = self.profile.temperature_table.min_value();
        const MIN_UNIVERSE_TEMPERATURE: f64 = kelvin_to_rankine(1.0);
        let mut t = t;
        if matches!(unit, TemperatureUnit::Celsius | TemperatureUnit::Kelvin) {
            t *= 1.80;
        }
        self.profile.temperature_bias = t;
        if min_std_atmosphere_temp + self.profile.temperature_bias < MIN_UNIVERSE_TEMPERATURE {
            let min_bias = MIN_UNIVERSE_TEMPERATURE - min_std_atmosphere_temp;
            tracing::warn!(
                "The temperature bias {} R is too low. It could result in temperatures below the absolute zero.\nTemperature bias is therefore capped to {}",
                format_g(self.profile.temperature_bias, 6),
                format_g(min_bias, 6)
            );
            self.profile.temperature_bias = min_bias;
        }
        self.calculate_pressure_breakpoints(self.sl_pressure);
        self.sl_temperature = self.get_temperature(0.0);
        self.calculate_sl_sound_speed_and_density();
    }

    /// `SetSLTemperatureGradedDelta`.
    pub fn set_sl_temperature_graded_delta(&mut self, unit: TemperatureUnit, deltemp: f64) {
        self.set_temperature_graded_delta(deltemp, 0.0, unit);
    }

    /// `SetTemperatureGradedDelta`: a delta at `h` (ft ASL) ramped out by
    /// the top of the table.
    pub fn set_temperature_graded_delta(&mut self, deltemp: f64, h: f64, unit: TemperatureUnit) {
        let min_std_atmosphere_temp = self.profile.temperature_table.min_value();
        let min_delta_temperature = min_std_atmosphere_temp - self.profile.std_sl_temperature;
        let mut deltemp = deltemp;
        if matches!(unit, TemperatureUnit::Celsius | TemperatureUnit::Kelvin) {
            deltemp *= 1.80;
        }
        if deltemp <= min_delta_temperature {
            tracing::warn!(
                "The temperature delta {} R is too low. It could result in temperatures below the absolute zero.\nTemperature delta is therefore capped to {}",
                format_g(deltemp, 6),
                format_g(min_delta_temperature, 6)
            );
            deltemp = min_delta_temperature;
        }
        self.profile.temperature_delta_gradient =
            deltemp / (self.profile.gradient_fadeout_altitude - self.geopotential_altitude(h));
        self.calculate_lapse_rates();
        self.calculate_pressure_breakpoints(self.sl_pressure);
        self.sl_temperature = self.get_temperature(0.0);
        self.calculate_sl_sound_speed_and_density();
    }

    /// `ResetSLTemperature`.
    pub fn reset_sl_temperature(&mut self) {
        self.profile.temperature_bias = 0.0;
        self.profile.temperature_delta_gradient = 0.0;
        self.calculate_lapse_rates();
        self.calculate_pressure_breakpoints(self.sl_pressure);
        self.sl_temperature = self.profile.std_sl_temperature;
        self.calculate_sl_sound_speed_and_density();
    }

    /// `CalculateLapseRates`: per-layer lapse rates of the table, less the
    /// graded delta.
    pub fn calculate_lapse_rates(&mut self) {
        let p = &mut self.profile;
        let table = &p.temperature_table;
        p.lapse_rates.clear();
        for bh in 0..table.num_rows() - 1 {
            let t0 = table.element(bh + 1, 1);
            let t1 = table.element(bh + 2, 1);
            let h0 = table.element(bh + 1, 0);
            let h1 = table.element(bh + 2, 0);
            p.lapse_rates
                .push((t1 - t0) / (h1 - h0) - p.temperature_delta_gradient);
        }
    }

    /// `CalculateSLDensity`.
    pub fn calculate_sl_density(&mut self) {
        self.sl_density = self.sl_pressure / (self.reng * self.sl_temperature);
    }

    /// `CalculateSLSoundSpeedAndDensity`.
    pub fn calculate_sl_sound_speed_and_density(&mut self) {
        self.sl_soundspeed = (SH_RATIO * self.reng * self.sl_temperature).sqrt();
        self.calculate_sl_density();
    }

    /// `PrintStandardAtmosphereTable`: the profile every 1000 ft, as text.
    /// Like JSBSim it leaves the atmosphere recalculated at
    /// `input.altitude_asl` by running the model again (`Run(false)`).
    pub fn print_standard_atmosphere_table(&mut self, overrides: &Overrides) -> String {
        let mut out = String::from(
            "Altitude (ft)   Temp (F)   Pressure (psf)   Density (sl/ft3)\n\
             -------------   --------   --------------   ----------------\n",
        );
        for i in (0..280000).step_by(1000) {
            self.calculate(f64::from(i), overrides);
            out.push_str(&format!(
                "{i:>12}  {:>9}  {:>13}  {:>18}\n",
                format_g(self.temperature - 459.67, 2),
                format_g(self.pressure, 4),
                format_g(self.density, 8)
            ));
        }
        if !self.base.skip() {
            self.calculate(self.input.altitude_asl, overrides);
        }
        out
    }
}
