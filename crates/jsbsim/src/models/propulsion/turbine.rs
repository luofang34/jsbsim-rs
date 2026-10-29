// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2003 David Culp (daveculp@cox.net)
// Derived from JSBSim src/models/propulsion/FGTurbine.{h,cpp}.
//
//! The turbine engine (`FGTurbine`): a phase machine (off, spin-up,
//! start, run, stall, seize, trim) over the spool speeds N1 and N2,
//! which lag the throttle at the spool-up and spool-down rates; the
//! thrust interpolates between the idle and military thrust of the
//! `IdleThrust` and `MilThrust` functions by N2, less the bleed, with
//! afterburner (`AugThrust`) and water injection (`Injection`), and the
//! fuel flow follows from the thrust specific fuel consumption.

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
use crate::functions::FnId;
use crate::xml::Element;

/// `FGTurbine::phaseType`.
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
    /// `tpStall`: compressor stall.
    Stall,
    /// `tpSeize`: the inner spool is seized.
    Seize,
    /// `tpTrim`: no time passes; the thrust is the steady one.
    Trim,
}

/// A spool rate: an engine function, or JSBSim's default (`FGSpoolUp`),
/// which scales `delay` down near idle and at altitude.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Spool {
    /// The pre-function `propulsion/engine[n]/N?Spool*`.
    Function(FnId),
    /// `FGSpoolUp(BPR, factor)`: `delay` is `factor * 90 / (BPR + 3)`.
    Default {
        /// Rate from idle to full, per s.
        delay: f64,
    },
}

/// What the engine file sets once.
#[derive(Debug, Clone)]
pub struct TurbineConfig {
    /// Unaugmented static sea-level thrust, lbs.
    pub mil_thrust: f64,
    /// Augmented static sea-level thrust, lbs.
    pub max_thrust: f64,
    /// Bypass ratio.
    pub bypass_ratio: f64,
    /// Thrust specific fuel consumption, lbs/h/lbf, before the
    /// temperature and N2 corrections (`FGSimplifiedTSFC`).
    pub tsfc: f64,
    /// Augmented thrust specific fuel consumption, lbs/h/lbf.
    pub atsfc: f64,
    /// N1 at idle, %.
    pub idle_n1: f64,
    /// N2 at idle, %.
    pub idle_n2: f64,
    /// N1 the starter reaches, %.
    pub ignition_n1: f64,
    /// N2 the starter reaches, %.
    pub ignition_n2: f64,
    /// N1 rate of the starter, %/s.
    pub n1_spinup: f64,
    /// N2 rate of the starter, %/s.
    pub n2_spinup: f64,
    /// N1 rate once lit, %/s.
    pub n1_start_rate: f64,
    /// N2 rate once lit, %/s.
    pub n2_start_rate: f64,
    /// N1 spin-down divisor.
    pub n1_spindown: f64,
    /// N2 spin-down divisor.
    pub n2_spindown: f64,
    /// 1 when an afterburner is fitted.
    pub augmented: i32,
    /// 0: `augmentation` property, 1: the last 1% of throttle, 2: throttle
    /// above 1.
    pub aug_method: i32,
    /// 1 when water injection is fitted.
    pub injected: i32,
    /// Water for this long, s.
    pub injection_time: f64,
    /// Inlets that close when off: no windmilling.
    pub disable_windmill: bool,
    /// Idle fuel flow, lbs/h.
    pub idle_ff: f64,
}

/// The engine functions `Calculate` reads.
#[derive(Debug, Clone)]
pub struct TurbineLookups {
    /// `IdleThrust`: idle over military thrust.
    pub idle_thrust: FnId,
    /// `MilThrust`: military thrust over its static sea-level value.
    pub mil_thrust: FnId,
    /// `AugThrust`: augmented thrust over its static sea-level value.
    pub aug_thrust: Option<FnId>,
    /// `Injection`: the thrust factor of water injection.
    pub injection: Option<FnId>,
    /// N1 rate up, %/s.
    pub n1_spool_up: Spool,
    /// N1 rate down, %/s.
    pub n1_spool_down: Spool,
    /// N2 rate up, %/s.
    pub n2_spool_up: Spool,
    /// N2 rate down, %/s.
    pub n2_spool_down: Spool,
}

/// What can go wrong or be commanded.
#[derive(Debug, Clone, Default)]
pub struct TurbineFlags {
    /// Compressor stall.
    pub stalled: bool,
    /// Inner spool seized.
    pub seized: bool,
    /// EGT over its limit.
    pub overtemp: bool,
    /// Fire.
    pub fire: bool,
    /// Water injection commanded.
    pub injection: bool,
    /// Afterburner lit.
    pub augmentation: bool,
    /// Reverser deployed.
    pub reversed: bool,
    /// Fuel cut off.
    pub cutoff: bool,
}

/// The state one `Calculate` updates.
#[derive(Debug, Clone)]
pub struct TurbineState {
    /// Operating phase.
    pub phase: Phase,
    /// %.
    pub n1: f64,
    /// %.
    pub n2: f64,
    /// N2 from idle (0) to full (1).
    pub n2norm: f64,
    /// N1 at full throttle, %.
    pub max_n1: f64,
    /// N2 at full throttle, %.
    pub max_n2: f64,
    /// `MaxN1 - IdleN1`.
    pub n1_factor: f64,
    /// `MaxN2 - IdleN2`.
    pub n2_factor: f64,
    /// The engine's throttle, at most 1.
    pub throttle_pos: f64,
    /// Throttle above 1 (the afterburner command of method 2).
    pub augment_cmd: f64,
    /// Exhaust gas temperature, °C.
    pub egt_degc: f64,
    /// Engine pressure ratio.
    pub epr: f64,
    /// psi.
    pub oil_pressure_psi: f64,
    /// K.
    pub oil_temp_degk: f64,
    /// Share of the thrust lost to bleed air.
    pub bleed_demand: f64,
    /// 1 open.
    pub inlet_position: f64,
    /// 1 open.
    pub nozzle_position: f64,
    /// Thrust specific fuel consumption after its corrections.
    pub corrected_tsfc: f64,
    /// Time water has been injected, s.
    pub injection_timer: f64,
    /// Water left, 1 full.
    pub inj_water_norm: f64,
    /// N1 added by water injection, %.
    pub inj_n1_increment: f64,
    /// N2 added by water injection, %.
    pub inj_n2_increment: f64,
}

/// `FGTurbine`'s members beyond `FGEngine`'s.
#[derive(Debug, Clone)]
pub struct Turbine {
    /// Engine number, for its inputs.
    pub number: usize,
    /// Set once.
    pub config: Box<TurbineConfig>,
    /// The engine functions.
    pub lookups: TurbineLookups,
    /// Failures and commands.
    pub flags: TurbineFlags,
    /// Updated each step.
    pub state: TurbineState,
}

/// What one `Calculate` reads: the engine functions' cached values (the
/// executive ran them as pre-functions), then the thruster's reads.
#[derive(Debug)]
pub struct TurbineSample {
    /// `IdleThrust`.
    pub idle_thrust: f64,
    /// `MilThrust`.
    pub mil_thrust: f64,
    /// `AugThrust`, when the engine has it.
    pub aug_thrust: f64,
    /// `Injection`, when the engine has it.
    pub injection: f64,
    /// The spool functions, by [`TurbineLookups`] order, NaN for a default.
    pub spools: [f64; 4],
    /// The thruster's reads.
    pub thruster: ThrusterSample,
}

/// `FGTurbine::Seek`: `v` moved toward `target` by `accel` or `decel` per
/// second over `dt`, without passing it.
#[must_use]
pub fn seek(dt: f64, v: f64, target: f64, accel: f64, decel: f64) -> f64 {
    let mut v = v;
    if v > target {
        v -= dt * decel;
        if v < target {
            v = target;
        }
    } else if v < target {
        v += dt * accel;
        if v > target {
            v = target;
        }
    }
    v
}

impl Turbine {
    /// `FGTurbine::ResetToIC` but for `FGEngine`'s.
    pub fn reset_state(&mut self, input: &Inputs) {
        let s = &mut self.state;
        s.n1 = 0.0;
        s.n2 = 0.0;
        s.inj_n1_increment = 0.0;
        s.inj_n2_increment = 0.0;
        s.n2norm = 0.0;
        let tsfc = self.tsfc(input);
        let s = &mut self.state;
        s.corrected_tsfc = tsfc;
        let s = &mut self.state;
        s.augment_cmd = 0.0;
        s.inj_water_norm = 0.0;
        s.inlet_position = 1.0;
        s.nozzle_position = 1.0;
        self.flags = TurbineFlags {
            cutoff: true,
            ..TurbineFlags::default()
        };
        let s = &mut self.state;
        s.phase = Phase::Off;
        s.egt_degc = input.tat_c;
        s.oil_temp_degk = input.tat_c + 273.0;
    }

    /// `FGSimplifiedTSFC::GetValue`: the TSFC corrected for the ambient
    /// temperature and N2.
    #[must_use]
    pub fn tsfc(&self, input: &Inputs) -> f64 {
        let n2norm = self.state.n2norm;
        self.config.tsfc
            * (input.temperature / 389.7).sqrt()
            * (0.84 + (1.0 - n2norm) * (1.0 - n2norm))
    }

    /// The value of `spool`: slot `i` of the sample for a function,
    /// `FGSpoolUp::GetValue` for the default.
    fn spool(&self, spool: Spool, sample: &TurbineSample, i: usize, input: &Inputs) -> f64 {
        match spool {
            Spool::Function(_) => sample.spools.get(i).copied().unwrap_or(f64::NAN),
            Spool::Default { delay } => {
                // `std::min(1.0, N2norm + 0.1)`.
                let x = self.state.n2norm + 0.1;
                let n = if x < 1.0 { x } else { 1.0 };
                delay
                    / (1.0 + 3.0 * (1.0 - n) * (1.0 - n) * (1.0 - n) + (1.0 - input.density_ratio))
            }
        }
    }

    /// `GetPowerAvailable`, as `FGTurbine` defines it.
    #[must_use]
    pub fn power_available(&self) -> f64 {
        let t = self.state.throttle_pos;
        if t <= 0.77 {
            64.94 * t
        } else {
            217.38 * t - 117.38
        }
    }

    /// `getOilTemp_degF`.
    #[must_use]
    pub fn oil_temp_degf(&self) -> f64 {
        crate::units::kelvin_to_fahrenheit(self.state.oil_temp_degk)
    }
}

fn function(ev: &mut ModelEvaluator<'_>, id: Option<FnId>) -> Result<f64, Error> {
    match id {
        Some(id) => ev.function(id),
        None => Ok(f64::NAN),
    }
}

fn spool_function(ev: &mut ModelEvaluator<'_>, spool: Spool) -> Result<f64, Error> {
    match spool {
        Spool::Function(id) => ev.function(id),
        Spool::Default { .. } => Ok(f64::NAN),
    }
}

impl EngineFamily for Turbine {
    const TYPE: EngineType = EngineType::Turbine;
    type Sample = TurbineSample;

    fn load(el: &Element, cx: &mut EngineLoad<'_>) -> Result<Self, Error> {
        load::load(el, cx)
    }

    fn sample(
        &self,
        _base: &EngineBase,
        thruster: &Thruster,
        models: &Models,
        ev: &mut ModelEvaluator<'_>,
    ) -> Result<TurbineSample, Error> {
        let l = &self.lookups;
        if l.injection.is_none() && self.config.injected == 1 && self.flags.injection {
            return Err(Error::Model {
                message: format!(
                    "engine {}: water injection commanded without an Injection function, which JSBSim dereferences",
                    self.number
                ),
            });
        }
        Ok(TurbineSample {
            idle_thrust: ev.function(l.idle_thrust)?,
            mil_thrust: ev.function(l.mil_thrust)?,
            aug_thrust: function(ev, l.aug_thrust)?,
            injection: function(ev, l.injection)?,
            spools: [
                spool_function(ev, l.n1_spool_up)?,
                spool_function(ev, l.n1_spool_down)?,
                spool_function(ev, l.n2_spool_up)?,
                spool_function(ev, l.n2_spool_down)?,
            ],
            thruster: thruster.sample(models, ev)?,
        })
    }

    /// `FGTurbine::Calculate` between the pre- and post-functions.
    fn step(
        &mut self,
        base: &mut EngineBase,
        thruster: &mut Thruster,
        sample: TurbineSample,
        input: &Inputs,
    ) {
        let s = &mut self.state;
        s.throttle_pos = input.throttle_pos.get(self.number).copied().unwrap_or(0.0);
        if s.throttle_pos > 1.0 {
            s.augment_cmd = s.throttle_pos - 1.0;
            s.throttle_pos -= s.augment_cmd;
        } else {
            s.augment_cmd = 0.0;
        }
        self.select_phase(base, input);
        let thrust = match self.state.phase {
            Phase::Off => self.off(base, input),
            Phase::Run => self.run(base, &sample, input),
            Phase::SpinUp => self.spin_up(base, input),
            Phase::Start => self.start(base, input),
            Phase::Stall => self.stall(base, input),
            Phase::Seize => self.seize(base, input),
            Phase::Trim => self.trim(&sample),
        };
        thruster.base.input.load(input);
        thruster.calculate(thrust, &sample.thruster);
    }

    /// `FGTurbine::CalcFuelNeed`: the fuel flow the phase set.
    fn calc_fuel_need(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
        base.fuel_flow_rate = base.fuel_flow_pph / 3600.0;
        base.fuel_expended = base.fuel_flow_rate * input.total_delta_t;
        if !base.starved {
            base.fuel_used_lbs += base.fuel_expended;
        }
        base.fuel_expended
    }

    /// `FGTurbine::InitRunning` up to its `Calculate`, which runs with
    /// integration suspended.
    fn init_running(
        &mut self,
        base: &mut EngineBase,
        _thruster: &mut Thruster,
        _input: &mut Inputs,
    ) -> InitRunning {
        let c = &self.config;
        let s = &mut self.state;
        self.flags.cutoff = false;
        base.running = true;
        s.n1_factor = s.max_n1 - c.idle_n1;
        s.n2_factor = s.max_n2 - c.idle_n2;
        s.n2 = c.idle_n2 + s.throttle_pos * s.n2_factor;
        s.n1 = c.idle_n1 + s.throttle_pos * s.n1_factor;
        InitRunning::Calculate {
            suspend_integration: true,
            zero_step: false,
        }
    }

    fn init_running_done(&mut self) {
        self.state.phase = Phase::Run;
    }

    /// `FGTurbine::ResetToIC`.
    fn reset_to_ic(&mut self, base: &mut EngineBase, thruster: &mut Thruster, input: &Inputs) {
        base.reset_to_ic();
        thruster.reset_to_ic();
        self.reset_state(input);
    }

    fn cutoff(&self) -> Option<bool> {
        Some(self.flags.cutoff)
    }

    fn set_cutoff(&mut self, cutoff: bool) {
        self.flags.cutoff = cutoff;
    }
}

crate::snapshot::visit_unit_enum! { Phase { Off, Run, SpinUp, Start, Stall, Seize, Trim } }
crate::snapshot::visit_fields! {
    TurbineConfig {
        mil_thrust, max_thrust, bypass_ratio, tsfc, atsfc, idle_n1, idle_n2, ignition_n1,
        ignition_n2, n1_spinup, n2_spinup, n1_start_rate, n2_start_rate, n1_spindown, n2_spindown,
        augmented, aug_method, injected, injection_time, disable_windmill, idle_ff,
    }
}
crate::snapshot::visit_fields! {
    TurbineFlags { stalled, seized, overtemp, fire, injection, augmentation, reversed, cutoff }
}
crate::snapshot::visit_fields! {
    TurbineState {
        phase, n1, n2, n2norm, max_n1, max_n2, n1_factor, n2_factor, throttle_pos, augment_cmd,
        egt_degc, epr, oil_pressure_psi, oil_temp_degk, bleed_demand, inlet_position,
        nozzle_position, corrected_tsfc, injection_timer, inj_water_norm, inj_n1_increment,
        inj_n2_increment,
    }
}
crate::snapshot::visit_fields! { Turbine { number, config, flags, state } skip { lookups } }
