// SPDX-License-Identifier: LGPL-2.1-or-later
// FGXMLElement: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// FGJSBBase: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/input_output/FGXMLElement.cpp (unit conversion
// table) and src/FGJSBBase.h (constants and temperature conversions).
//
//! Unit conversion factors and physical constants, computed with the same
//! expressions in the same order as JSBSim so every factor is bit-identical.

use std::collections::BTreeMap;
use std::f64::consts::PI;

#[cfg(test)]
mod tests;

/// `radtodeg`.
pub const RADTODEG: f64 = 180. / PI;
/// `degtorad`.
pub const DEGTORAD: f64 = PI / 180.;
/// `hptoftlbssec`.
pub const HPTOFTLBSSEC: f64 = 550.0;
/// `psftoinhg`.
pub const PSFTOINHG: f64 = 0.014138;
/// `psftopa`.
pub const PSFTOPA: f64 = 47.88;
/// `fttom`.
pub const FTTOM: f64 = 0.3048;
/// `ktstofps`.
pub const KTSTOFPS: f64 = 1852. / (3600.0 * FTTOM);
/// `fpstokts`.
pub const FPSTOKTS: f64 = 1.0 / KTSTOFPS;
/// `inchtoft`.
pub const INCHTOFT: f64 = 1.0 / 12.0;
/// `m3toft3`.
pub const M3TOFT3: f64 = 1.0 / (FTTOM * FTTOM * FTTOM);
/// `in3tom3`.
pub const IN3TOM3: f64 = INCHTOFT * INCHTOFT * INCHTOFT / M3TOFT3;
/// `inhgtopa`.
pub const INHGTOPA: f64 = 3386.38;
/// `slugtolb`.
pub const SLUGTOLB: f64 = 32.174049;
/// `lbtoslug`.
pub const LBTOSLUG: f64 = 1.0 / SLUGTOLB;
/// `kgtolb`.
pub const KGTOLB: f64 = 2.20462;
/// `kgtoslug`.
pub const KGTOSLUG: f64 = 0.06852168;

/// `KelvinToFahrenheit`.
#[must_use]
pub fn kelvin_to_fahrenheit(kelvin: f64) -> f64 {
    1.8 * kelvin - 459.4
}

/// `CelsiusToRankine`.
#[must_use]
pub const fn celsius_to_rankine(celsius: f64) -> f64 {
    celsius * 1.8 + 491.67
}

/// `RankineToCelsius`.
#[must_use]
pub const fn rankine_to_celsius(rankine: f64) -> f64 {
    (rankine - 491.67) / 1.8
}

/// `KelvinToRankine`.
#[must_use]
pub const fn kelvin_to_rankine(kelvin: f64) -> f64 {
    kelvin * 1.8
}

/// `RankineToKelvin`.
#[must_use]
pub fn rankine_to_kelvin(rankine: f64) -> f64 {
    rankine / 1.8
}

/// `FahrenheitToCelsius`.
#[must_use]
pub fn fahrenheit_to_celsius(fahrenheit: f64) -> f64 {
    (fahrenheit - 32.0) / 1.8
}

/// `CelsiusToFahrenheit`.
#[must_use]
pub fn celsius_to_fahrenheit(celsius: f64) -> f64 {
    celsius * 1.8 + 32.0
}

/// `CelsiusToKelvin`.
#[must_use]
pub fn celsius_to_kelvin(celsius: f64) -> f64 {
    celsius + 273.15
}

/// `KelvinToCelsius`.
#[must_use]
pub fn kelvin_to_celsius(kelvin: f64) -> f64 {
    kelvin - 273.15
}

/// `FeetToMeters`.
#[must_use]
pub fn feet_to_meters(measure: f64) -> f64 {
    measure * 0.3048
}

/// `EqualToRoundoff(double, double)`.
#[must_use]
pub fn equal_to_roundoff(a: f64, b: f64) -> bool {
    let eps = 2.0 * f64::EPSILON;
    (a - b).abs() <= eps * a.abs().max(b.abs())
}

/// `Constrain(min, value, max)`.
#[must_use]
pub fn constrain(min: f64, value: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// `sign`: 1 for non-negative numbers, -1 otherwise.
#[must_use]
pub fn sign(num: f64) -> f64 {
    if num >= 0.0 { 1.0 } else { -1.0 }
}

/// The `convert[from][to]` table of `FGXMLElement`: `value_in_from *
/// factor == value_in_to`.
#[derive(Debug, Clone)]
pub struct UnitTable {
    factors: BTreeMap<(&'static str, &'static str), f64>,
}

impl Default for UnitTable {
    fn default() -> Self {
        Self::new()
    }
}

impl UnitTable {
    /// Builds the table.
    #[must_use]
    pub fn new() -> Self {
        let mut t = Builder(BTreeMap::new());
        t.dimensioned();
        t.identities();
        Self { factors: t.0 }
    }

    /// Factor from `from` to `to`, if JSBSim defines one.
    #[must_use]
    pub fn factor(&self, from: &str, to: &str) -> Option<f64> {
        self.factors.get(&(from, to)).copied()
    }

    /// Whether `unit` appears as a source unit.
    #[must_use]
    pub fn knows(&self, unit: &str) -> bool {
        self.factors.keys().any(|(from, _)| *from == unit)
    }

    /// Every `(from, to, factor)`, sorted by unit names.
    pub fn entries(&self) -> impl Iterator<Item = (&'static str, &'static str, f64)> + '_ {
        self.factors.iter().map(|(&(from, to), &f)| (from, to, f))
    }
}

struct Builder(BTreeMap<(&'static str, &'static str), f64>);

impl Builder {
    fn set(&mut self, from: &'static str, to: &'static str, factor: f64) {
        self.0.insert((from, to), factor);
    }

    /// `convert[from][to]`; like `std::map::operator[]`, 0 for a missing
    /// entry (none of JSBSim's expressions reads one).
    fn get(&self, from: &str, to: &str) -> f64 {
        self.0.get(&(from, to)).copied().unwrap_or(0.0)
    }

    fn dimensioned(&mut self) {
        self.length_area_volume();
        self.mass_angle_force();
        self.velocity_pressure_flow();
    }

    fn length_area_volume(&mut self) {
        self.set("M", "FT", 3.2808399);
        self.set("FT", "M", 1.0 / self.get("M", "FT"));
        self.set("CM", "FT", 0.032808399);
        self.set("FT", "CM", 1.0 / self.get("CM", "FT"));
        self.set("KM", "FT", 3280.8399);
        self.set("FT", "KM", 1.0 / self.get("KM", "FT"));
        self.set("FT", "IN", 12.0);
        self.set("IN", "FT", 1.0 / self.get("FT", "IN"));
        self.set("IN", "M", self.get("IN", "FT") * self.get("FT", "M"));
        self.set("M", "IN", self.get("M", "FT") * self.get("FT", "IN"));
        self.set("M2", "FT2", self.get("M", "FT") * self.get("M", "FT"));
        self.set("FT2", "M2", 1.0 / self.get("M2", "FT2"));
        self.set("CM2", "FT2", self.get("CM", "FT") * self.get("CM", "FT"));
        self.set("FT2", "CM2", 1.0 / self.get("CM2", "FT2"));
        self.set("M2", "IN2", self.get("M", "IN") * self.get("M", "IN"));
        self.set("IN2", "M2", 1.0 / self.get("M2", "IN2"));
        self.set("FT2", "IN2", 144.0);
        self.set("IN2", "FT2", 1.0 / self.get("FT2", "IN2"));
        self.set("IN3", "CC", 16.387064);
        self.set("CC", "IN3", 1.0 / self.get("IN3", "CC"));
        self.set("FT3", "IN3", 1728.0);
        self.set("IN3", "FT3", 1.0 / self.get("FT3", "IN3"));
        self.set("M3", "FT3", 35.3146667);
        self.set("FT3", "M3", 1.0 / self.get("M3", "FT3"));
        self.set("LTR", "IN3", 61.0237441);
        self.set("IN3", "LTR", 1.0 / self.get("LTR", "IN3"));
        self.set("GAL", "FT3", 0.133681);
        self.set("FT3", "GAL", 1.0 / self.get("GAL", "FT3"));
        self.set(
            "IN3",
            "GAL",
            self.get("IN3", "FT3") * self.get("FT3", "GAL"),
        );
        self.set(
            "LTR",
            "GAL",
            self.get("LTR", "IN3") * self.get("IN3", "GAL"),
        );
        self.set("M3", "GAL", 1000. * self.get("LTR", "GAL"));
        self.set("CC", "GAL", self.get("CC", "IN3") * self.get("IN3", "GAL"));
    }

    fn mass_angle_force(&mut self) {
        self.set("LBS", "KG", 0.45359237);
        self.set("KG", "LBS", 1.0 / self.get("LBS", "KG"));
        self.set("SLUG", "KG", 14.59390);
        self.set("KG", "SLUG", 1.0 / self.get("SLUG", "KG"));
        self.set(
            "SLUG",
            "LBS",
            self.get("SLUG", "KG") / self.get("LBS", "KG"),
        );
        self.set("LBS", "SLUG", 1.0 / self.get("SLUG", "LBS"));
        self.set("SLUG*FT2", "KG*M2", 1.35594);
        self.set("KG*M2", "SLUG*FT2", 1.0 / self.get("SLUG*FT2", "KG*M2"));
        self.set("RAD", "DEG", 180.0 / PI);
        self.set("DEG", "RAD", 1.0 / self.get("RAD", "DEG"));
        self.set("RAD/SEC", "DEG/SEC", self.get("RAD", "DEG"));
        self.set("DEG/SEC", "RAD/SEC", 1.0 / self.get("RAD/SEC", "DEG/SEC"));
        self.set("LBS/FT", "N/M", 14.5939);
        self.set("N/M", "LBS/FT", 1.0 / self.get("LBS/FT", "N/M"));
        self.set("LBS/FT/SEC", "N/M/SEC", 14.5939);
        self.set(
            "N/M/SEC",
            "LBS/FT/SEC",
            1.0 / self.get("LBS/FT/SEC", "N/M/SEC"),
        );
        self.set("LBS/FT2/SEC2", "N/M2/SEC2", 47.880259);
        let f = 1.0 / self.get("LBS/FT2/SEC2", "N/M2/SEC2");
        self.set("N/M2/SEC2", "LBS/FT2/SEC2", f);
        self.set("WATTS", "HP", 0.001341022);
        self.set("HP", "WATTS", 1.0 / self.get("WATTS", "HP"));
        self.set("N", "LBS", 0.22482);
        self.set("LBS", "N", 1.0 / self.get("N", "LBS"));
    }

    fn velocity_pressure_flow(&mut self) {
        self.set("KTS", "FT/SEC", 1.6878098571);
        self.set("FT/SEC", "KTS", 1.0 / self.get("KTS", "FT/SEC"));
        self.set("M/S", "FT/S", 3.2808399);
        self.set(
            "M/S",
            "KTS",
            self.get("M/S", "FT/S") / self.get("KTS", "FT/SEC"),
        );
        self.set("M/SEC", "FT/SEC", 3.2808399);
        self.set("FT/S", "M/S", 1.0 / self.get("M/S", "FT/S"));
        self.set("M/SEC", "FT/SEC", 3.2808399);
        self.set("FT/SEC", "M/SEC", 1.0 / self.get("M/SEC", "FT/SEC"));
        self.set("KM/SEC", "FT/SEC", 3280.8399);
        self.set("FT/SEC", "KM/SEC", 1.0 / self.get("KM/SEC", "FT/SEC"));
        self.set("FT*LBS", "N*M", 1.35581795);
        self.set("N*M", "FT*LBS", 1.0 / self.get("FT*LBS", "N*M"));
        let m_ft = self.get("M", "FT");
        let valve = m_ft * m_ft * m_ft * m_ft / self.get("KG", "SLUG");
        self.set("M4*SEC/KG", "FT4*SEC/SLUG", valve);
        self.set("FT4*SEC/SLUG", "M4*SEC/KG", 1.0 / valve);
        self.set("INHG", "PSF", 70.7180803);
        self.set("PSF", "INHG", 1.0 / self.get("INHG", "PSF"));
        self.set("ATM", "INHG", 29.9246899);
        self.set("INHG", "ATM", 1.0 / self.get("ATM", "INHG"));
        self.set("PSI", "INHG", 2.03625437);
        self.set("INHG", "PSI", 1.0 / self.get("PSI", "INHG"));
        self.set("INHG", "PA", 3386.0);
        self.set("PA", "INHG", 1.0 / self.get("INHG", "PA"));
        self.set("LBS/FT2", "N/M2", 14.5939 / self.get("FT", "M"));
        self.set("N/M2", "LBS/FT2", 1.0 / self.get("LBS/FT2", "N/M2"));
        self.set("LBS/FT2", "PA", self.get("LBS/FT2", "N/M2"));
        self.set("PA", "LBS/FT2", 1.0 / self.get("LBS/FT2", "PA"));
        self.set("KG/MIN", "LBS/MIN", self.get("KG", "LBS"));
        self.set("KG/SEC", "LBS/SEC", self.get("KG", "LBS"));
        self.set("N/SEC", "LBS/SEC", 0.224808943);
        self.set("LBS/SEC", "N/SEC", 1.0 / self.get("N/SEC", "LBS/SEC"));
        self.set("LBS/HP*HR", "KG/KW*HR", 0.6083);
        self.set(
            "KG/KW*HR",
            "LBS/HP*HR",
            1.0 / self.get("LBS/HP*HR", "KG/KW*HR"),
        );
        self.set("KG/L", "LBS/GAL", 8.3454045);
        self.set("LBS/GAL", "KG/L", 1.0 / self.get("KG/L", "LBS/GAL"));
        self.set("FT3/SEC2", "M3/SEC2", self.get("FT3", "M3"));
        self.set("M3/SEC2", "FT3/SEC2", self.get("M3", "FT3"));
    }

    fn identities(&mut self) {
        for unit in [
            "M",
            "KM",
            "FT",
            "IN",
            "M2",
            "FT2",
            "IN3",
            "CC",
            "M3",
            "FT3",
            "LTR",
            "GAL",
            "KG",
            "LBS",
            "KG*M2",
            "SLUG*FT2",
            "DEG",
            "RAD",
            "DEG/SEC",
            "RAD/SEC",
            "LBS/FT",
            "N/M",
            "LBS/FT/SEC",
            "N/M/SEC",
            "LBS/FT2/SEC2",
            "N/M2/SEC2",
            "HP",
            "WATTS",
            "N",
            "FT/SEC",
            "KTS",
            "M/S",
            "M/SEC",
            "KM/SEC",
            "FT*LBS",
            "N*M",
            "M4*SEC/KG",
            "FT4*SEC/SLUG",
            "PSI",
            "PSF",
            "INHG",
            "ATM",
            "PA",
            "N/M2",
            "LBS/FT2",
            "LBS/SEC",
            "KG/MIN",
            "LBS/MIN",
            "N/SEC",
            "LBS/HP*HR",
            "KG/KW*HR",
            "KG/L",
            "LBS/GAL",
            "FT3/SEC2",
            "M3/SEC2",
            "VOLTS",
            "OHMS",
            "AMPERES",
        ] {
            self.set(unit, unit, 1.0);
        }
    }
}
