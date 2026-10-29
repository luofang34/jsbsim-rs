// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/atmosphere/FGStandardAtmosphere.cpp
// (pressure, density and their altitude inversions).
//
//! Pressure and density of the 1976 standard atmosphere (equations 33a and
//! 33b of the U.S. Standard Atmosphere document), and the pressure and
//! density altitudes they invert to.

use super::standard::RDRY;
use super::{Atmosphere, AtmosphereModel, G0, PressureUnit};

/// Layer `b` of a per-layer profile; NaN past its end, where JSBSim reads
/// past its vector.
fn layer(values: &[f64], b: usize) -> f64 {
    values.get(b).copied().unwrap_or(f64::NAN)
}

/// Pressure at `deltah` above a layer base of temperature `tmb`, lapse
/// rate `lmb` and pressure `pb`.
fn layer_pressure(pb: f64, tmb: f64, lmb: f64, delta_h: f64) -> f64 {
    if lmb != 0.0 {
        let exp = G0 / (RDRY * lmb);
        let factor = tmb / (tmb + lmb * delta_h);
        pb * libm::pow(factor, exp)
    } else {
        pb * crate::elementary::exp(-G0 * delta_h / (RDRY * tmb))
    }
}

impl Atmosphere {
    /// The layer of geopotential altitude `geo_pot_alt`: its index and base
    /// altitude.
    fn layer_of(&self, geo_pot_alt: f64) -> (usize, f64) {
        let table = &self.profile.temperature_table;
        let mut base_alt = table.element(1, 0);
        let num_rows = table.num_rows();
        let mut b = 0;
        while b < num_rows - 2 {
            let test_alt = table.element(b + 2, 0);
            if geo_pot_alt < test_alt {
                break;
            }
            base_alt = test_alt;
            b += 1;
        }
        (b, base_alt)
    }

    /// `GetPressure(altitude)`: modeled pressure at `altitude` (ft ASL),
    /// psf.
    #[must_use]
    pub fn get_pressure(&self, altitude: f64) -> f64 {
        match self.model {
            AtmosphereModel::Mars => return self.pressure,
            AtmosphereModel::Msis => return self.msis_compute(altitude).pressure,
            AtmosphereModel::Standard => {}
        }
        let geo_pot_alt = self.geopotential_altitude(altitude);
        let (b, base_alt) = self.layer_of(geo_pot_alt);
        let tmb = self.get_temperature(self.geometric_altitude(base_alt));
        let delta_h = geo_pot_alt - base_alt;
        let p = &self.profile;
        let lmb = layer(&p.lapse_rates, b);
        layer_pressure(layer(&p.pressure_breakpoints, b), tmb, lmb, delta_h)
    }

    /// `GetStdPressure(altitude)`, psf. Like JSBSim it uses the current
    /// (graded) lapse rates with the standard breakpoints.
    #[must_use]
    pub fn get_std_pressure(&self, altitude: f64) -> f64 {
        let geo_pot_alt = self.geopotential_altitude(altitude);
        let (b, base_alt) = self.layer_of(geo_pot_alt);
        let tmb = self.get_std_temperature(self.geometric_altitude(base_alt));
        let delta_h = geo_pot_alt - base_alt;
        let p = &self.profile;
        let lmb = layer(&p.lapse_rates, b);
        layer_pressure(layer(&p.std_pressure_breakpoints, b), tmb, lmb, delta_h)
    }

    /// `GetStdDensity(altitude)`, slug/ft^3.
    #[must_use]
    pub fn get_std_density(&self, altitude: f64) -> f64 {
        self.get_std_pressure(altitude) / (RDRY * self.get_std_temperature(altitude))
    }

    /// `SetPressureSL`: rescales the pressure profile. The sea-level speed
    /// of sound is left as it is.
    pub fn set_pressure_sl(&mut self, unit: PressureUnit, pressure: f64) {
        // `FGMSIS` overrides it with a no-op.
        if self.model == AtmosphereModel::Msis {
            return;
        }
        let p = Self::convert_to_psf(pressure, unit);
        self.sl_pressure = Self::validate_pressure(p, "Sea Level pressure", false);
        self.calculate_sl_density();
        self.calculate_pressure_breakpoints(self.sl_pressure);
    }

    /// `ResetSLPressure`.
    pub fn reset_sl_pressure(&mut self) {
        self.sl_pressure = self.profile.std_sl_pressure;
        self.calculate_sl_density();
        self.calculate_pressure_breakpoints(self.profile.std_sl_pressure);
    }

    /// `CalculatePressureBreakpoints`: pressure at each layer base given
    /// `sl_press` and the current bias and gradient.
    pub fn calculate_pressure_breakpoints(&mut self, sl_press: f64) {
        let p = &mut self.profile;
        let table = &p.temperature_table;
        // Each breakpoint is the pressure at the top of the layer below.
        let mut pb = sl_press;
        for (i, slot) in p.pressure_breakpoints.iter_mut().enumerate() {
            if let Some(b) = i.checked_sub(1) {
                let base_temp = table.element(b + 1, 1);
                let base_alt = table.element(b + 1, 0);
                let upper_alt = table.element(b + 2, 0);
                let delta_h = upper_alt - base_alt;
                let tmb = base_temp
                    + p.temperature_bias
                    + (p.gradient_fadeout_altitude - base_alt) * p.temperature_delta_gradient;
                pb = layer_pressure(pb, tmb, layer(&p.lapse_rates, b), delta_h);
            }
            *slot = pb;
        }
    }

    /// `CalculateStdDensityBreakpoints`.
    pub fn calculate_std_density_breakpoints(&mut self) {
        let p = &mut self.profile;
        p.std_density_breakpoints.clear();
        for (i, &pb) in p.std_pressure_breakpoints.iter().enumerate() {
            p.std_density_breakpoints
                .push(pb / (RDRY * p.temperature_table.element(i + 1, 1)));
        }
    }

    /// `CalculateDensityAltitude`: the standard-day altitude (ft) with
    /// `density`. `_geometric_alt` is unused, as in JSBSim.
    #[must_use]
    pub fn calculate_density_altitude(&self, density: f64, _geometric_alt: f64) -> f64 {
        let p = &self.profile;
        let mut b = 0;
        while b < p.std_density_breakpoints.len() - 2 {
            if p.std_density_breakpoints
                .get(b + 1)
                .is_some_and(|&d| density >= d)
            {
                break;
            }
            b += 1;
        }
        let tmb = p.temperature_table.element(b + 1, 1);
        let hb = p.temperature_table.element(b + 1, 0);
        let lmb = layer(&p.std_lapse_rates, b);
        let pb = layer(&p.std_density_breakpoints, b);
        // The barometric formula for density, solved for H.
        let density_altitude = if lmb != 0.0 {
            let exp = -1.0 / (1.0 + G0 / (RDRY * lmb));
            hb + (tmb / lmb) * (libm::pow(density / pb, exp) - 1.0)
        } else {
            let factor = -RDRY * tmb / G0;
            hb + factor * libm::log(density / pb)
        };
        self.geometric_altitude(density_altitude)
    }

    /// `CalculatePressureAltitude`: the standard-day altitude (ft) with
    /// `pressure`. `_geometric_alt` is unused, as in JSBSim.
    #[must_use]
    pub fn calculate_pressure_altitude(&self, pressure: f64, _geometric_alt: f64) -> f64 {
        let p = &self.profile;
        let mut b = 0;
        while b < p.std_pressure_breakpoints.len() - 2 {
            if p.std_pressure_breakpoints
                .get(b + 1)
                .is_some_and(|&pb| pressure >= pb)
            {
                break;
            }
            b += 1;
        }
        let tmb = p.temperature_table.element(b + 1, 1);
        let hb = p.temperature_table.element(b + 1, 0);
        let lmb = layer(&p.std_lapse_rates, b);
        let pb = layer(&p.std_pressure_breakpoints, b);
        // Equations 33(a) and 33(b) solved for H.
        let pressure_altitude = if lmb != 0.00 {
            let exp = -RDRY * lmb / G0;
            hb + (tmb / lmb) * (libm::pow(pressure / pb, exp) - 1.0)
        } else {
            let factor = -RDRY * tmb / G0;
            hb + factor * libm::log(pressure / pb)
        };
        self.geometric_altitude(pressure_altitude)
    }
}
