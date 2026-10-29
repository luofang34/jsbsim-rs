// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGAuxiliary.cpp (PitotTotalPressure,
// MachFromImpactPressure, VcalibratedFromMach, MachFromVcalibrated).
//
//! Pitot-static relations between Mach number, impact pressure and
//! calibrated airspeed, after the USAF Aircraft Performance Flight Testing
//! Manual (AFFTC-TIH-99-01), sections 4.6 to 4.8.
//!
//! JSBSim defines them on `FGAuxiliary` only; `FGInitialCondition` calls
//! the auxiliary's, so both models call these. The calibrated-airspeed
//! relations read the auxiliary's `in.StdDaySLsoundspeed`, which
//! `FGFDMExec::LoadPlanetConstants` copies from the constant
//! `FGAtmosphere::StdDaySLsoundspeed`; callers pass that speed.

use super::Auxiliary;
use crate::models::atmosphere::{SH_RATIO, STD_DAY_SL_PRESSURE};

/// `coeff` of [`pitot_total_pressure`]: `pow(0.5*(SHRatio+1), b) *
/// pow((SHRatio+1)/(SHRatio-1), d)`. Clang folds the `pow`s of these
/// constant operands at compile time with the platform `pow`, which rounds
/// each of them correctly; Rust cannot fold `libm::pow`, so the value is
/// spelled out bit for bit, and a test checks it against the expression
/// evaluated with the platform `pow`.
pub(super) const PITOT_COEFF: f64 = f64::from_bits(0x4064_dd7d_9589_19b5);

/// `coeff` of [`mach_from_impact_pressure`]: `pow(0.5*(SHRatio+1),
/// -0.25*c) * pow(0.5*(SHRatio+1)/SHRatio, -0.5*d)`, spelled out as
/// [`PITOT_COEFF`] is. The `libm` crate's `pow` gives a product one ulp
/// lower.
pub(super) const RAYLEIGH_COEFF: f64 = f64::from_bits(0x3fec_337c_4b7c_389c);

/// `PitotTotalPressure(mach, pressure)`, psf: isentropic below Mach 1,
/// Rayleigh's pitot formula behind a normal shock above it. A negative
/// Mach number gives the static pressure.
#[must_use]
pub fn pitot_total_pressure(mach: f64, pressure: f64) -> f64 {
    let a = (SH_RATIO - 1.0) / 2.0;
    let b = SH_RATIO / (SH_RATIO - 1.0);
    let c = 2.0 * b;
    let d = 1.0 / (SH_RATIO - 1.0);

    if mach < 0.0 {
        return pressure;
    }
    if mach < 1.0 {
        pressure * crate::elementary::pow(1.0 + a * mach * mach, b)
    } else {
        // The denominator vanishes near Mach 0.38, well inside the
        // subsonic branch.
        pressure * PITOT_COEFF * crate::elementary::pow(mach, c)
            / crate::elementary::pow(c * mach * mach - 1.0, d)
    }
}

/// `MachFromImpactPressure(qc, pressure)`: equation 4.12, refined above
/// Mach 1 by ten iterations of equation 4.17.
#[must_use]
pub fn mach_from_impact_pressure(qc: f64, pressure: f64) -> f64 {
    let a = 2.0 / (SH_RATIO - 1.0);
    let b = (SH_RATIO - 1.0) / SH_RATIO;
    let c = 2.0 / b;
    let d = 0.5 * a;

    let big_a = qc / pressure + 1.0;
    let mut m = (a * (crate::elementary::pow(big_a, b) - 1.0)).sqrt();
    if m > 1.0 {
        for _ in 0..10 {
            m = RAYLEIGH_COEFF
                * (big_a * crate::elementary::pow(1.0 - 1.0 / (c * m * m), d)).sqrt();
        }
    }
    m
}

/// `VcalibratedFromMach(mach, pressure)`: calibrated airspeed, ft/s, at
/// static pressure `pressure`, psf, for the standard-day sea-level speed
/// of sound `std_day_sl_soundspeed`, ft/s.
#[must_use]
pub fn vcalibrated_from_mach(mach: f64, pressure: f64, std_day_sl_soundspeed: f64) -> f64 {
    vcalibrated_from_total_pressure(
        pitot_total_pressure(mach, pressure),
        pressure,
        std_day_sl_soundspeed,
    )
}

/// [`vcalibrated_from_mach`] given the pitot total pressure `pt` it
/// computes first, for callers that already have it.
#[must_use]
pub(super) fn vcalibrated_from_total_pressure(
    pt: f64,
    pressure: f64,
    std_day_sl_soundspeed: f64,
) -> f64 {
    let qc = pt - pressure;
    std_day_sl_soundspeed * mach_from_impact_pressure(qc, STD_DAY_SL_PRESSURE)
}

/// `MachFromVcalibrated(vcas, pressure)`: the Mach number at static
/// pressure `pressure`, psf, for calibrated airspeed `vcas`, ft/s, and the
/// standard-day sea-level speed of sound `std_day_sl_soundspeed`, ft/s.
#[must_use]
pub fn mach_from_vcalibrated(vcas: f64, pressure: f64, std_day_sl_soundspeed: f64) -> f64 {
    let qc = pitot_total_pressure(vcas / std_day_sl_soundspeed, STD_DAY_SL_PRESSURE)
        - STD_DAY_SL_PRESSURE;
    mach_from_impact_pressure(qc, pressure)
}

impl Auxiliary {
    /// [`vcalibrated_from_mach`] with this model's speed of sound.
    #[must_use]
    pub fn vcalibrated_from_mach(&self, mach: f64, pressure: f64) -> f64 {
        vcalibrated_from_mach(mach, pressure, self.input.constants.std_day_sl_soundspeed)
    }

    /// [`mach_from_vcalibrated`] with this model's speed of sound.
    #[must_use]
    pub fn mach_from_vcalibrated(&self, vcas: f64, pressure: f64) -> f64 {
        mach_from_vcalibrated(vcas, pressure, self.input.constants.std_day_sl_soundspeed)
    }
}
