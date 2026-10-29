// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2004 Jiri "Javky" Javurek (javky@email.cz)
// Derived from JSBSim src/models/propulsion/FGTurboProp.{h,cpp}.
//
//! The turboprop engine (`FGTurboProp`): a phase machine (off, spin-up,
//! start, run, trim) over the gas generator speed N1, which follows the
//! throttle with exponential lags; the power comes from the
//! `EnginePowerRPM_N1` table and the `EnginePowerVC` correction, the
//! fuel flow from the power specific fuel consumption and the combustion
//! efficiency, and the inter-turbine temperature from `ITT_N1`. The
//! propeller can be reversed through the beta range, and an electronic
//! limiter (IELU) can hold the throttle back at a torque limit.

mod bind;
mod load;
mod phases;
#[cfg(test)]
mod tests;

use super::Inputs;
use super::engine::{EngineBase, EngineType};
use super::families::{EngineFamily, InitRunning, ThrusterSample};
use super::thruster::Thruster;
use crate::Error;
use crate::fdm::ModelEvaluator;
use crate::fdm::{EngineLoad, Models};
use crate::functions::{FnId, Table, TableId};
use crate::units::HPTOFTLBSSEC;
use crate::xml::Element;

/// `FGTurboProp::phaseType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// `tpOff`.
    Off,
    /// `tpRun`.
    Run,
    /// `tpSpinUp`: the starter turns the engine.
    SpinUp,
    /// `tpStart`: lit, accelerating to idle.
    Start,
    /// `tpTrim`: no time passes and the engine gives no power.
    Trim,
}

/// The `EnginePowerVC` correction: the engine's pre-function, or (the
/// deprecated form) a table bound to `propulsion/engine[n]/EnginePowerVC`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerVc {
    /// `<function name="EnginePowerVC">`.
    Function(FnId),
    /// `<table name="EnginePowerVC">`.
    Table(TableId),
}

/// What the engine file sets once.
#[derive(Debug, Clone)]
pub struct TurboPropConfig {
    /// N1 at idle, %.
    pub idle_n1: f64,
    /// N1 at full throttle, %.
    pub max_n1: f64,
    /// `MaxN1 - IdleN1`.
    pub n1_factor: f64,
    /// Throttle where the beta range ends when reversed, 0 to 1.
    pub beta_range_throttle_end: f64,
    /// Share of full power reversed, 0 to 1.
    pub reverse_max_power: f64,
    /// hp.
    pub max_power: f64,
    /// Power specific fuel consumption, lbs/hp/h.
    pub psfc: f64,
    /// Time constant of the N1 lag, s.
    pub idle_max_delay: f64,
    /// A start not done by then fails, s; negative for none.
    pub max_starting_time: f64,
    /// N1 the starter reaches, %.
    pub starter_n1: f64,
    /// Propeller torque the IELU holds; negative for no IELU.
    pub ielu_max_torque: f64,
    /// Time constant of the ITT lag, s.
    pub itt_delay: f64,
    /// Power by RPM and N1, hp.
    pub engine_power_rpm_n1: Table,
    /// ITT by N1 and a second key, °C.
    pub itt_n1: Table,
    /// Combustion efficiency by N1.
    pub combustion_efficiency_n1: Table,
    /// The power correction for speed and altitude.
    pub engine_power_vc: PowerVc,
}

/// The state one `Calculate` updates.
#[derive(Debug, Clone)]
pub struct TurboPropState {
    /// Operating phase.
    pub phase: Phase,
    /// %.
    pub n1: f64,
    /// Engine power, hp.
    pub hp: f64,
    /// The engine's throttle after the beta range and the IELU.
    pub throttle_pos: f64,
    /// `throttle_pos` of the last IELU check.
    pub old_throttle: f64,
    /// Shaft speed, rpm.
    pub rpm: f64,
    /// psi.
    pub oil_pressure_psi: f64,
    /// K.
    pub oil_temp_degk: f64,
    /// Combustion efficiency of the last fuel flow.
    pub combustion_efficiency: f64,
    /// Time since the starter engaged, s; negative when not starting.
    pub start_time: f64,
    /// Inter-turbine temperature, °C.
    pub eng_itt_degc: f64,
    /// Temperature inside the engine, °C.
    pub eng_temperature: f64,
    /// Reverser engaged.
    pub reversed: bool,
    /// Fuel cut off.
    pub cutoff: bool,
    /// The IELU holds the throttle back.
    pub ielu_intervent: bool,
    /// Starting.
    pub eng_starting: bool,
    /// The starter has power.
    pub generator_power: bool,
    /// 10 and above shuts the engine down.
    pub condition: i32,
}

/// `FGTurboProp`'s members beyond `FGEngine`'s.
#[derive(Debug, Clone)]
pub struct TurboProp {
    /// Engine number, for its inputs.
    pub number: usize,
    /// Set once.
    pub config: Box<TurboPropConfig>,
    /// Updated each step.
    pub state: TurboPropState,
}

/// What one `Calculate` reads: `EnginePowerVC`, then the thruster's reads.
#[derive(Debug)]
pub struct TurboPropSample {
    /// `EnginePowerVC->GetValue()`.
    pub power_vc: f64,
    /// The thruster's reads.
    pub thruster: ThrusterSample,
}

/// `FGTurboProp::Seek`, which is `FGTurbine::Seek`.
pub use super::turbine::seek;

/// `FGTurboProp::ExpSeek`: `v` moved toward `target` by a first-order lag
/// with time constant `accel_tau` or `decel_tau` over `dt`.
#[must_use]
pub fn exp_seek(dt: f64, v: f64, target: f64, accel_tau: f64, decel_tau: f64) -> f64 {
    if v > target {
        (v - target) * libm::exp(-dt / decel_tau) + target
    } else if v < target {
        (target - v) * (1.0 - libm::exp(-dt / accel_tau)) + v
    } else {
        v
    }
}

/// Input `i` of `v`, 0 where the vector is short.
fn at(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(0.0)
}

impl TurboProp {
    /// `getOilTemp_degF`.
    #[must_use]
    pub fn oil_temp_degf(&self) -> f64 {
        crate::units::kelvin_to_fahrenheit(self.state.oil_temp_degk)
    }

    /// The propeller's controls, and the throttle through the beta range
    /// when reversed.
    fn set_propeller(&mut self, thruster: &mut Thruster, input: &Inputs) {
        if !thruster.is_propeller() {
            return;
        }
        let (c, s) = (&self.config, &mut self.state);
        thruster.set_advance(at(&input.prop_advance, self.number));
        thruster.set_feather(
            input
                .prop_feather
                .get(self.number)
                .copied()
                .unwrap_or(false),
        );
        thruster.set_reverse(s.reversed);
        thruster.set_reverse_coef(if s.reversed { s.throttle_pos } else { 0.0 });
        if s.reversed {
            let end = c.beta_range_throttle_end;
            s.throttle_pos = if s.throttle_pos < end {
                // Idle in the beta range.
                0.0
            } else {
                (s.throttle_pos - end) / (1.0 - end) * c.reverse_max_power
            };
        }
    }

    /// The IELU: above the torque limit the throttle may only come back.
    fn limit_torque(&mut self, thruster: &Thruster, dt: f64) {
        let s = &mut self.state;
        if self.config.ielu_max_torque <= 0.0 {
            return;
        }
        let torque = thruster.torque();
        if s.condition < 1 {
            if torque.abs() > self.config.ielu_max_torque && s.throttle_pos >= s.old_throttle {
                s.throttle_pos = s.old_throttle - 0.1 * dt;
                s.ielu_intervent = true;
            } else if s.ielu_intervent && s.throttle_pos >= s.old_throttle {
                s.throttle_pos = s.old_throttle + 0.05 * dt;
                s.ielu_intervent = true;
            } else {
                s.ielu_intervent = false;
            }
        } else {
            s.ielu_intervent = false;
        }
        s.old_throttle = s.throttle_pos;
    }
}

impl EngineFamily for TurboProp {
    const TYPE: EngineType = EngineType::Turboprop;
    type Sample = TurboPropSample;

    fn load(el: &Element, cx: &mut EngineLoad<'_>) -> Result<Self, Error> {
        load::load(el, cx)
    }

    fn sample(
        &self,
        _base: &EngineBase,
        thruster: &Thruster,
        models: &Models,
        ev: &mut ModelEvaluator<'_>,
    ) -> Result<TurboPropSample, Error> {
        let power_vc = match self.config.engine_power_vc {
            PowerVc::Function(id) => ev.function(id)?,
            PowerVc::Table(id) => ev.table(id)?,
        };
        Ok(TurboPropSample {
            power_vc,
            thruster: thruster.sample(models, ev)?,
        })
    }

    /// `FGTurboProp::Calculate` between the pre- and post-functions.
    fn step(
        &mut self,
        base: &mut EngineBase,
        thruster: &mut Thruster,
        sample: TurboPropSample,
        input: &Inputs,
    ) {
        let dt = input.total_delta_t;
        self.state.throttle_pos = at(&input.throttle_pos, self.number);
        // The thruster holds the gear ratio and the rest of the
        // transmission.
        self.state.rpm = thruster.engine_rpm();
        self.set_propeller(thruster, input);
        self.select_phase(base, input);
        self.limit_torque(thruster, dt);
        let hp = match self.state.phase {
            Phase::Off => self.off(base, input),
            Phase::Run => self.run(base, sample.power_vc, input),
            Phase::SpinUp => self.spin_up(base, sample.power_vc, input),
            Phase::Start => self.start(base, sample.power_vc, input),
            Phase::Trim => 0.0,
        };
        self.state.hp = hp;
        thruster.base.input.load(input);
        let mut power = hp * HPTOFTLBSSEC;
        // No negative power while the propeller is not turning:
        // `std::max(power, 0.0)`.
        if self.state.rpm <= 0.1 && power < 0.0 {
            power = 0.0;
        }
        thruster.calculate(power, &sample.thruster);
    }

    /// `FGTurboProp::CalcFuelNeed`: the fuel flow the phase set.
    fn calc_fuel_need(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
        base.fuel_flow_rate = base.fuel_flow_pph / 3600.0;
        base.fuel_expended = base.fuel_flow_rate * input.total_delta_t;
        if !base.starved {
            base.fuel_used_lbs += base.fuel_expended;
        }
        base.fuel_expended
    }

    /// `FGTurboProp::InitRunning` up to its `Calculate`, which runs with
    /// the engine inputs' step at 0.
    fn init_running(
        &mut self,
        base: &mut EngineBase,
        _thruster: &mut Thruster,
        _input: &mut Inputs,
    ) -> InitRunning {
        self.state.cutoff = false;
        base.running = true;
        InitRunning::Calculate {
            suspend_integration: false,
            zero_step: true,
        }
    }

    fn cutoff(&self) -> Option<bool> {
        Some(self.state.cutoff)
    }

    fn set_cutoff(&mut self, cutoff: bool) {
        self.state.cutoff = cutoff;
    }
}
