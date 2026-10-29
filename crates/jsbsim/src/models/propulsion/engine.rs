// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGEngine.{h,cpp}.
//
//! What every engine has (`FGEngine`'s members). Engine types are
//! registered in [`families`](super::families).

use super::Inputs;
use super::families::EngineKind;
use super::thruster::Thruster;
use crate::fdm::ModelFunctions;

/// `FGEngine::EngineType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineType {
    /// `etUnknown`.
    Unknown,
    /// `etRocket`: burns oxidizer too.
    Rocket,
    /// `etPiston`.
    Piston,
    /// `etTurbine`.
    Turbine,
    /// `etTurboprop`.
    Turboprop,
    /// `etElectric`.
    Electric,
}

/// `FGEngine`'s members.
#[derive(Debug, Clone)]
pub struct EngineBase {
    /// `name` attribute of the engine type's element.
    pub name: String,
    /// Index among the propulsion's engines.
    pub number: usize,
    /// The type's `EngineType`.
    pub engine_type: EngineType,
    /// Sea-level fuel flow at full power, lbs/s.
    pub sl_fuel_flow_max: f64,
    /// Throttle range.
    pub max_throttle: f64,
    /// See `max_throttle`.
    pub min_throttle: f64,
    /// Fuel burnt in the last step, lbs.
    pub fuel_expended: f64,
    /// Fuel flow, lbs/s.
    pub fuel_flow_rate: f64,
    /// Fraction of full power.
    pub pct_power: f64,
    /// Starter engaged.
    pub starter: bool,
    /// No tank can feed it.
    pub starved: bool,
    /// Running.
    pub running: bool,
    /// Cranking.
    pub cranking: bool,
    /// Burns no fuel.
    pub fuel_freeze: bool,
    /// Fuel flow, gal/h, as engine types compute it.
    pub fuel_flow_gph: f64,
    /// Fuel flow, lbs/h, as engine types compute it.
    pub fuel_flow_pph: f64,
    /// Fuel burnt since the last reset, lbs.
    pub fuel_used_lbs: f64,
    /// Density of the fuel, lbs/gal.
    pub fuel_density: f64,
    /// The tanks that feed it, by index.
    pub source_tanks: Vec<i32>,
    /// Its `<property>` declarations and pre- and post-functions
    /// (`FGModelFunctions`, with the engine number as prefix).
    pub functions: ModelFunctions,
}

impl EngineBase {
    /// `FGEngine(engine_number, input)`.
    #[must_use]
    pub fn new(number: usize, engine_type: EngineType) -> Self {
        Self {
            name: String::new(),
            number,
            engine_type,
            sl_fuel_flow_max: 0.0,
            max_throttle: 1.0,
            min_throttle: 0.0,
            fuel_expended: 0.0,
            fuel_flow_rate: 0.0,
            pct_power: 0.0,
            starter: false,
            starved: false,
            running: false,
            cranking: false,
            fuel_freeze: false,
            fuel_flow_gph: 0.0,
            fuel_flow_pph: 0.0,
            fuel_used_lbs: 0.0,
            fuel_density: 6.02,
            source_tanks: Vec::new(),
            functions: ModelFunctions::default(),
        }
    }

    /// `FGEngine::ResetToIC` but for the thruster's.
    pub fn reset_to_ic(&mut self) {
        self.starter = false;
        self.fuel_expended = 0.0;
        self.starved = false;
        self.running = false;
        self.cranking = false;
        self.pct_power = 0.0;
        self.fuel_flow_gph = 0.0;
        self.fuel_flow_pph = 0.0;
        self.fuel_flow_rate = 0.0;
        self.fuel_freeze = false;
        self.fuel_used_lbs = 0.0;
    }

    /// `FGEngine::CalcFuelNeed`: the fuel burnt over the step at the
    /// current power.
    pub fn calc_fuel_need(&mut self, input: &Inputs) -> f64 {
        self.fuel_flow_rate = self.sl_fuel_flow_max * self.pct_power;
        self.fuel_expended = self.fuel_flow_rate * input.total_delta_t;
        if !self.starved {
            self.fuel_used_lbs += self.fuel_expended;
        }
        self.fuel_expended
    }

    /// `GetFuelFlowRateGPH`.
    #[must_use]
    pub fn fuel_flow_rate_gph(&self) -> f64 {
        self.fuel_flow_rate * 3600.0 / self.fuel_density
    }
}

/// An engine: what every engine has, its thruster, and its type's own
/// state.
#[derive(Debug, Clone)]
pub struct Engine {
    /// What every engine has.
    pub base: EngineBase,
    /// Its thruster.
    pub thruster: Thruster,
    /// The engine type and its state.
    pub kind: EngineKind,
}
