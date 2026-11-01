// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGAtmosphere.h and
// src/models/atmosphere/FGStandardAtmosphere.h (their constants).
//
//! The constants of the air and the standard day.

use crate::units::{FTTOM, KGTOSLUG, kelvin_to_rankine};

/// `StdDaySLtemperature`, °R.
pub(crate) const STD_DAY_SL_TEMPERATURE: f64 = 518.67;
/// `StdDaySLpressure`, psf.
pub(crate) const STD_DAY_SL_PRESSURE: f64 = 2116.228;
/// `SHRatio`: ratio of specific heats of air.
pub(crate) const SH_RATIO: f64 = 1.4;
/// `Rstar`: universal gas constant, ft·lbf/R/mol.
pub(crate) const RSTAR: f64 = 8.31432 * KGTOSLUG / kelvin_to_rankine(FTTOM * FTTOM);
/// `Mair`: mean molecular weight of air, slug/mol.
pub(crate) const MAIR: f64 = 28.9645 * KGTOSLUG / 1000.0;
/// `g0`: the ISA's sea-level gravity, ft/s^2, whatever gravity model runs.
pub(crate) const G0: f64 = 9.80665 / FTTOM;
/// `Reng0`: specific gas constant of dry air, ft·lbf/slug/R.
pub(crate) const RENG0: f64 = RSTAR / MAIR;
/// `StdDaySLdensity`, slug/ft^3.
pub(crate) const STD_DAY_SL_DENSITY: f64 = STD_DAY_SL_PRESSURE / (RENG0 * STD_DAY_SL_TEMPERATURE);
/// `SutherlandConstant`, °R.
pub(crate) const SUTHERLAND_CONSTANT: f64 = 198.72;
/// `Beta`, slug/(s·ft·R^0.5).
pub(crate) const BETA: f64 = 2.269690E-08;
