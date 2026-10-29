// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/atmosphere/FGMSIS.{h,cpp}.
//
//! The NRLMSISE-00 atmosphere (`FGMSIS`): a standard atmosphere whose
//! temperature, density and gas constant come from [`nrlmsise::gtd7`] at
//! the aircraft's position and the time of day.

use super::nrlmsise::{self, Flags, Input};
use super::{Atmosphere, AtmosphereModel, BETA, RSTAR, SH_RATIO, SUTHERLAND_CONSTANT};
use crate::Error;
use crate::units::{FTTOM, KGTOSLUG, M3TOFT3, kelvin_to_rankine};
use crate::xml::Element;

/// Molecular weights, g/mol, of N2, O2, O, He, H, Ar, N and anomalous O.
const SPECIES_MMOL: [f64; 8] = [
    28.0134,
    31.9988,
    31.9988 / 2.0,
    4.0,
    1.0,
    39.948,
    28.0134 / 2.0,
    31.9988 / 2.0,
];

/// `FGMSIS`'s own state: the date and the solar and magnetic activity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Msis {
    /// `day_of_year`.
    pub day_of_year: f64,
    /// `seconds_in_day`, UTC.
    pub seconds_in_day: f64,
    /// `flags`: every switch on.
    pub flags: Flags,
    /// `input.f107A`.
    pub f107a: f64,
    /// `input.f107`.
    pub f107: f64,
    /// `input.ap`.
    pub ap: f64,
}

impl Default for Msis {
    fn default() -> Self {
        Self {
            day_of_year: 1.0,
            seconds_in_day: 0.0,
            flags: Flags::default(),
            f107a: 150.0,
            f107: 150.0,
            ap: 4.0,
        }
    }
}

/// What `FGMSIS::Compute` gives at one altitude.
#[derive(Debug, Clone, Copy)]
pub(super) struct Computed {
    pub(super) pressure: f64,
    pub(super) temperature: f64,
    pub(super) density: f64,
    pub(super) rair: f64,
}

impl Atmosphere {
    /// `FGMSIS(fdmex)`: a fresh standard atmosphere run by NRLMSISE-00.
    #[must_use]
    pub fn msis() -> Self {
        Self {
            model: AtmosphereModel::Msis,
            ..Self::default()
        }
    }

    /// `FGMSIS::Load`: the `<day>` of the year and `<utc>` seconds.
    pub fn load_msis(&mut self, el: &Element) -> Result<(), Error> {
        if el.find_element("day").is_some() {
            self.msis.day_of_year = el.find_element_value_as_number("day")?;
        }
        if el.find_element("utc").is_some() {
            self.msis.seconds_in_day = el.find_element_value_as_number("utc")?;
        }
        Ok(())
    }

    /// `FGMSIS::Compute(altitude)`: pressure, temperature, density and the
    /// gas constant of the air at `altitude` ft above the position the
    /// executive last loaded, at the current simulation time.
    pub(super) fn msis_compute(&self, altitude: f64) -> Computed {
        const FTTOKM: f64 = FTTOM / 1000.;
        const KGM3_TO_SLUGFT3: f64 = KGTOSLUG / M3TOFT3;
        const GTOSLUG: f64 = KGTOSLUG / 1000.;
        let m = &self.msis;
        let h = altitude * FTTOKM;
        let lat = self.input.geod_latitude_deg;
        let lon = self.input.longitude_deg;

        let mut utc_seconds = m.seconds_in_day + self.sim_time;
        // `unsigned int` conversions, which truncate.
        let days = (utc_seconds / 86400.) as u32;
        utc_seconds -= f64::from(days) * 86400.;
        let mut today = m.day_of_year + f64::from(days);
        let year = (today / 365.) as u32;
        today -= f64::from(year) * 365.;

        let input = Input {
            year: 0,
            doy: today as i32,
            sec: utc_seconds,
            alt: h,
            g_lat: lat,
            g_long: lon,
            lst: utc_seconds / 3600. + lon / 15.,
            f107a: m.f107a,
            f107: m.f107,
            ap: m.ap,
            ap_a: None,
        };
        let output = nrlmsise::gtd7(&input, &m.flags).unwrap_or(nrlmsise::Output {
            d: [f64::NAN; 9],
            t: [f64::NAN; 2],
        });

        let temperature = kelvin_to_rankine(output.t[1]);
        let density = output.d[5] * KGM3_TO_SLUGFT3;
        // N2, O2, O, He, H, Ar, N; GTD7 leaves anomalous oxygen out of the
        // total density, so it is left out of the molar mass too.
        let d = output.d;
        let dn = [d[2], d[3], d[1], d[0], d[6], d[4], d[7], 0.0];
        let mut mmol = 0.0;
        let mut qty_mol = 0.0;
        for (n, mm) in dn.iter().zip(SPECIES_MMOL) {
            mmol += n * mm;
            qty_mol += n;
        }
        let mair = mmol * GTOSLUG / qty_mol;
        let rair = RSTAR / mair;
        Computed {
            pressure: density * rair * temperature,
            temperature,
            density,
            rair,
        }
    }

    /// `FGMSIS::Calculate`: sea level and the altitude from the model;
    /// pressure and density altitudes from the standard atmosphere's
    /// breakpoints. No override property is read, nor any humidity.
    pub(super) fn msis_calculate(&mut self, altitude: f64) {
        let sl = self.msis_compute(0.0);
        self.sl_pressure = sl.pressure;
        self.sl_temperature = sl.temperature;
        self.sl_density = sl.density;
        let here = self.msis_compute(altitude);
        self.pressure = here.pressure;
        self.temperature = here.temperature;
        self.density = here.density;
        self.reng = here.rair;
        self.sl_soundspeed = (SH_RATIO * sl.rair * self.sl_temperature).sqrt();
        self.sound_speed = (SH_RATIO * self.reng * self.temperature).sqrt();
        self.pressure_altitude = self.calculate_pressure_altitude(self.pressure, altitude);
        self.density_altitude = self.calculate_density_altitude(self.density, altitude);
        self.viscosity = BETA * crate::elementary::pow(self.temperature, 1.5)
            / (SUTHERLAND_CONSTANT + self.temperature);
        self.kinematic_viscosity = self.viscosity / self.density;
    }

    /// `FGMSIS::GetSoundSpeed(altitude)`.
    pub(super) fn msis_sound_speed(&self, altitude: f64) -> f64 {
        let c = self.msis_compute(altitude);
        (SH_RATIO * c.rair * c.temperature).sqrt()
    }
}
