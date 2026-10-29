// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2003 David Culp (daveculp@cox.net)
// Derived from JSBSim src/models/propulsion/FGTurbine.cpp (Off, Run,
// SpinUp, Start, Stall, Seize, Trim).
//
//! The phases of a turbine: each updates the engine and returns the
//! thrust, lbs.

use super::{Phase, Turbine, TurbineSample, seek};
use crate::models::propulsion::Inputs;
use crate::models::propulsion::engine::EngineBase;

impl Turbine {
    /// The phase `Calculate` picks before it runs one.
    pub(super) fn select_phase(&mut self, base: &EngineBase, input: &Inputs) {
        let c = &self.config;
        let (s, f) = (&mut self.state, &mut self.flags);
        // When trimming is finished, the engine is left off or running.
        if s.phase == Phase::Trim && input.total_delta_t > 0.0 {
            if base.running && !base.starved {
                s.phase = Phase::Run;
                s.n1_factor = s.max_n1 - c.idle_n1;
                s.n2_factor = s.max_n2 - c.idle_n2;
                s.n2 = c.idle_n2 + s.throttle_pos * s.n2_factor;
                s.n1 = c.idle_n1 + s.throttle_pos * s.n1_factor;
                s.oil_temp_degk = 366.0;
                f.cutoff = false;
            } else {
                s.phase = Phase::Off;
                f.cutoff = true;
                s.egt_degc = input.tat_c;
            }
        }
        if !base.running && f.cutoff && base.starter && s.phase == Phase::Off {
            s.phase = Phase::SpinUp;
        }
        if (base.starter || input.qbar > 30.0) && !base.running && !f.cutoff && s.n2 > 15.0 {
            s.phase = Phase::Start;
        }
        if f.cutoff && s.phase != Phase::SpinUp {
            s.phase = Phase::Off;
        }
        if input.total_delta_t == 0.0 {
            s.phase = Phase::Trim;
        }
        if base.starved {
            s.phase = Phase::Off;
        }
        if f.stalled {
            s.phase = Phase::Stall;
        }
        if f.seized {
            s.phase = Phase::Seize;
        }
    }

    /// `Off`: the spools wind down to their windmilling speeds.
    pub(super) fn off(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
        let dt = input.total_delta_t;
        let c = &self.config;
        let s = &mut self.state;
        base.running = false;
        base.fuel_flow_pph = seek(dt, base.fuel_flow_pph, 0.0, 1000.0, 10000.0);
        if c.disable_windmill {
            s.n1 = seek(dt, s.n1, 0.0, s.n1 / 2.0, s.n1 / c.n1_spindown);
            s.n2 = seek(dt, s.n2, 0.0, s.n2 / 2.0, s.n2 / c.n2_spindown);
        } else {
            // A small acceleration even at N = 0.
            s.n1 = seek(
                dt,
                s.n1,
                input.qbar / 10.0,
                s.n1 / 2.0 + 0.1,
                s.n1 / c.n1_spindown,
            );
            s.n2 = seek(
                dt,
                s.n2,
                input.qbar / 15.0,
                s.n2 / 2.0 + 0.1,
                s.n2 / c.n2_spindown,
            );
        }
        s.egt_degc = seek(dt, s.egt_degc, input.tat_c, 11.7, 7.3);
        s.oil_temp_degk = seek(dt, s.oil_temp_degk, input.tat_c + 273.0, 0.2, 0.2);
        s.oil_pressure_psi = s.n2 * 0.62;
        s.nozzle_position = seek(dt, s.nozzle_position, 1.0, 0.8, 0.8);
        s.epr = seek(dt, s.epr, 1.0, 0.2, 0.2);
        self.flags.augmentation = false;
        0.0
    }

    /// `Run`.
    pub(super) fn run(
        &mut self,
        base: &mut EngineBase,
        sample: &TurbineSample,
        input: &Inputs,
    ) -> f64 {
        let dt = input.total_delta_t;
        let mil = self.config.mil_thrust;
        let idlethrust = mil * sample.idle_thrust;
        let milthrust = (mil - idlethrust) * sample.mil_thrust;
        base.running = true;
        base.starter = false;
        let injecting = self.spool_to_throttle(sample, input);
        let s = &mut self.state;
        s.n2norm = (s.n2 - self.config.idle_n2) / s.n2_factor;
        let mut thrust = idlethrust + (milthrust * s.n2norm * s.n2norm);
        s.egt_degc = input.tat_c + 363.1 + s.n2norm * 357.1;
        s.oil_pressure_psi = s.n2 * 0.62;
        s.oil_temp_degk = seek(dt, s.oil_temp_degk, 366.0, 1.2, 0.1);
        if !self.flags.augmentation {
            let tsfc = self.tsfc(input);
            let s = &mut self.state;
            s.corrected_tsfc = tsfc;
            base.fuel_flow_pph = seek(dt, base.fuel_flow_pph, thrust * tsfc, 1000.0, 10000.0);
            if base.fuel_flow_pph < self.config.idle_ff {
                base.fuel_flow_pph = self.config.idle_ff;
            }
            s.nozzle_position = seek(dt, s.nozzle_position, 1.0 - s.n2norm, 0.8, 0.8);
            thrust *= 1.0 - s.bleed_demand;
            s.epr = 1.0 + thrust / mil;
        }
        thrust = self.augment(base, thrust, sample.aug_thrust, dt);
        let (c, s) = (&self.config, &mut self.state);
        if injecting && s.inj_water_norm > 0.0 {
            s.injection_timer += dt;
            if s.injection_timer < c.injection_time {
                thrust *= sample.injection;
                s.inj_water_norm = 1.0 - (s.injection_timer / c.injection_time);
            } else {
                self.flags.injection = false;
                s.inj_water_norm = 0.0;
            }
        }
        if self.flags.cutoff || base.starved {
            s.phase = Phase::Off;
        }
        thrust
    }

    /// The spools of `Run` moved toward the throttle's speeds, faster with
    /// water injected; whether water is being injected.
    fn spool_to_throttle(&mut self, sample: &TurbineSample, input: &Inputs) -> bool {
        let dt = input.total_delta_t;
        let injecting = self.config.injected == 1 && self.flags.injection;
        {
            let c = &self.config;
            let s = &mut self.state;
            s.n1_factor = s.max_n1 - c.idle_n1;
            s.n2_factor = s.max_n2 - c.idle_n2;
            if injecting && s.inj_water_norm > 0.0 {
                s.n1_factor += s.inj_n1_increment;
                s.n2_factor += s.inj_n2_increment;
            }
        }
        let l = &self.lookups;
        let (n2_up, n2_down) = (
            self.spool(l.n2_spool_up, sample, 2, input),
            self.spool(l.n2_spool_down, sample, 3, input),
        );
        let s = &mut self.state;
        s.n2 = seek(
            dt,
            s.n2,
            self.config.idle_n2 + s.throttle_pos * s.n2_factor,
            n2_up,
            n2_down,
        );
        let (n1_up, n1_down) = (
            self.spool(l.n1_spool_up, sample, 0, input),
            self.spool(l.n1_spool_down, sample, 1, input),
        );
        let s = &mut self.state;
        s.n1 = seek(
            dt,
            s.n1,
            self.config.idle_n1 + s.throttle_pos * s.n1_factor,
            n1_up,
            n1_down,
        );
        injecting
    }

    /// The afterburner part of `Run`.
    fn augment(&mut self, base: &mut EngineBase, thrust: f64, aug_thrust: f64, dt: f64) -> f64 {
        let (c, s, f) = (&self.config, &mut self.state, &mut self.flags);
        let mut thrust = thrust;
        if c.aug_method == 1 {
            f.augmentation = s.throttle_pos > 0.99 && s.n2 > 97.0;
        }
        if c.augmented == 1 && f.augmentation && c.aug_method < 2 {
            thrust = aug_thrust * c.max_thrust;
            base.fuel_flow_pph = seek(dt, base.fuel_flow_pph, thrust * c.atsfc, 5000.0, 10000.0);
            s.nozzle_position = seek(dt, s.nozzle_position, 1.0, 0.8, 0.8);
        }
        if c.aug_method == 2 {
            if s.augment_cmd > 0.0 {
                f.augmentation = true;
                let tdiff = (c.max_thrust * aug_thrust) - thrust;
                thrust += tdiff * min_one(s.augment_cmd);
                base.fuel_flow_pph =
                    seek(dt, base.fuel_flow_pph, thrust * c.atsfc, 5000.0, 10000.0);
                s.nozzle_position = seek(dt, s.nozzle_position, 1.0, 0.8, 0.8);
            } else {
                f.augmentation = false;
            }
        }
        thrust
    }

    /// `SpinUp`: the starter turns the spools to their ignition speeds.
    pub(super) fn spin_up(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
        let dt = input.total_delta_t;
        let (c, s) = (&self.config, &mut self.state);
        base.running = false;
        base.fuel_flow_pph = 0.0;
        s.n2 = seek(dt, s.n2, c.ignition_n2, c.n2_spinup, s.n2 / 2.0);
        s.n1 = seek(dt, s.n1, c.ignition_n1, c.n1_spinup, s.n1 / 2.0);
        s.egt_degc = seek(dt, s.egt_degc, input.tat_c, 11.7, 7.3);
        s.oil_pressure_psi = s.n2 * 0.62;
        s.oil_temp_degk = seek(dt, s.oil_temp_degk, input.tat_c + 273.0, 0.2, 0.2);
        s.epr = 1.0;
        s.nozzle_position = 1.0;
        if !base.starter {
            s.phase = Phase::Off;
        }
        0.0
    }

    /// `Start`: lit at 15% N2 or more, accelerating to idle.
    pub(super) fn start(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
        let dt = input.total_delta_t;
        let (c, s) = (&self.config, &mut self.state);
        if s.n2 > 15.0 && !base.starved {
            base.cranking = true;
            if s.n2 < c.idle_n2 {
                s.n2 = seek(dt, s.n2, c.idle_n2, c.n2_start_rate, s.n2 / 2.0);
                s.n1 = seek(dt, s.n1, c.idle_n1, c.n1_start_rate, s.n1 / 2.0);
                s.egt_degc = seek(dt, s.egt_degc, input.tat_c + 363.1, 21.3, 7.3);
                base.fuel_flow_pph = c.idle_ff * s.n2 / c.idle_n2;
                s.oil_pressure_psi = s.n2 * 0.62;
                if !base.starter && input.qbar < 30.0 {
                    // An aborted start.
                    s.phase = Phase::Off;
                }
            } else {
                s.phase = Phase::Run;
                base.running = true;
                base.starter = false;
                base.cranking = false;
            }
        } else {
            s.phase = Phase::Off;
            base.starter = false;
        }
        0.0
    }

    /// `Stall`: cleared by bringing the throttle to idle.
    pub(super) fn stall(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
        let dt = input.total_delta_t;
        let s = &mut self.state;
        s.egt_degc = input.tat_c + 903.14;
        base.fuel_flow_pph = self.config.idle_ff;
        s.n1 = seek(dt, s.n1, input.qbar / 10.0, 0.0, s.n1 / 10.0);
        s.n2 = seek(dt, s.n2, input.qbar / 15.0, 0.0, s.n2 / 10.0);
        if s.throttle_pos < 0.01 {
            s.phase = Phase::Run;
            self.flags.stalled = false;
        }
        0.0
    }

    /// `Seize`.
    pub(super) fn seize(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
        let dt = input.total_delta_t;
        let s = &mut self.state;
        s.n2 = 0.0;
        s.n1 = seek(dt, s.n1, input.qbar / 20.0, 0.0, s.n1 / 15.0);
        base.fuel_flow_pph = if self.flags.cutoff {
            0.0
        } else {
            self.config.idle_ff
        };
        s.oil_pressure_psi = 0.0;
        s.oil_temp_degk = seek(dt, s.oil_temp_degk, input.tat_c + 273.0, 0.0, 0.2);
        base.running = false;
        0.0
    }

    /// `Trim`: the thrust at the throttle's steady N2, which only sets the
    /// afterburner flag.
    pub(super) fn trim(&mut self, sample: &TurbineSample) -> f64 {
        let (c, s, f) = (&self.config, &self.state, &mut self.flags);
        let idlethrust = c.mil_thrust * sample.idle_thrust;
        let milthrust = (c.mil_thrust - idlethrust) * sample.mil_thrust;
        let n2 = c.idle_n2 + s.throttle_pos * s.n2_factor;
        let n2norm = (n2 - c.idle_n2) / s.n2_factor;
        let mut thrust = (idlethrust + (milthrust * n2norm * n2norm)) * (1.0 - s.bleed_demand);
        if c.aug_method == 1 {
            f.augmentation = s.throttle_pos > 0.99 && n2 > 97.0;
        }
        if c.augmented == 1 && f.augmentation && c.aug_method < 2 {
            thrust = c.max_thrust * sample.aug_thrust;
        }
        if c.aug_method == 2 && s.augment_cmd > 0.0 {
            let tdiff = (c.max_thrust * sample.aug_thrust) - thrust;
            thrust += tdiff * min_one(s.augment_cmd);
        }
        if c.injected == 1 && f.injection {
            thrust *= sample.injection;
        }
        thrust
    }
}

/// `std::min(x, 1.0)`.
fn min_one(x: f64) -> f64 {
    if 1.0 < x { 1.0 } else { x }
}
