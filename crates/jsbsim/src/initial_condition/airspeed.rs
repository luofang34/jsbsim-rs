// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGAuxiliary.cpp (PitotTotalPressure,
// MachFromImpactPressure, VcalibratedFromMach, MachFromVcalibrated).
//
//! Pitot-static relations between Mach number, calibrated airspeed and
//! pressure, after the USAF Aircraft Performance Flight Testing Manual
//! (AFFTC-TIH-99-01), sections 4.6 to 4.8.

use crate::models::atmosphere::{SH_RATIO, STD_DAY_SL_PRESSURE};

/// `PitotTotalPressure(mach, pressure)`: isentropic below Mach 1, behind a
/// normal shock (Rayleigh pitot formula) above.
#[must_use]
pub fn pitot_total_pressure(mach: f64, pressure: f64) -> f64 {
    let a = (SH_RATIO - 1.0) / 2.0;
    let b = SH_RATIO / (SH_RATIO - 1.0);
    let c = 2.0 * b;
    let d = 1.0 / (SH_RATIO - 1.0);
    let coeff =
        libm::pow(0.5 * (SH_RATIO + 1.0), b) * libm::pow((SH_RATIO + 1.0) / (SH_RATIO - 1.0), d);
    if mach < 0.0 {
        return pressure;
    }
    if mach < 1.0 {
        pressure * libm::pow(1.0 + a * mach * mach, b)
    } else {
        pressure * coeff * libm::pow(mach, c) / libm::pow(c * mach * mach - 1.0, d)
    }
}

/// `MachFromImpactPressure(qc, pressure)`: equation 4.12, refined with ten
/// iterations of 4.17 when supersonic.
#[must_use]
pub fn mach_from_impact_pressure(qc: f64, pressure: f64) -> f64 {
    let a = 2.0 / (SH_RATIO - 1.0);
    let b = (SH_RATIO - 1.0) / SH_RATIO;
    let c = 2.0 / b;
    let d = 0.5 * a;
    let coeff = libm::pow(0.5 * (SH_RATIO + 1.0), -0.25 * c)
        * libm::pow(0.5 * (SH_RATIO + 1.0) / SH_RATIO, -0.5 * d);
    let big_a = qc / pressure + 1.0;
    let mut m = (a * (libm::pow(big_a, b) - 1.0)).sqrt();
    if m > 1.0 {
        for _ in 0..10 {
            m = coeff * (big_a * libm::pow(1.0 - 1.0 / (c * m * m), d)).sqrt();
        }
    }
    m
}

/// `VcalibratedFromMach(mach, pressure)`, ft/s, for the standard-day sea
/// level speed of sound `std_day_sl_soundspeed`.
#[must_use]
pub fn vcalibrated_from_mach(mach: f64, pressure: f64, std_day_sl_soundspeed: f64) -> f64 {
    let qc = pitot_total_pressure(mach, pressure) - pressure;
    std_day_sl_soundspeed * mach_from_impact_pressure(qc, STD_DAY_SL_PRESSURE)
}

/// `MachFromVcalibrated(vcas, pressure)`, for the standard-day sea level
/// speed of sound `std_day_sl_soundspeed`.
#[must_use]
pub fn mach_from_vcalibrated(vcas: f64, pressure: f64, std_day_sl_soundspeed: f64) -> f64 {
    let qc = pitot_total_pressure(vcas / std_day_sl_soundspeed, STD_DAY_SL_PRESSURE)
        - STD_DAY_SL_PRESSURE;
    mach_from_impact_pressure(qc, pressure)
}
