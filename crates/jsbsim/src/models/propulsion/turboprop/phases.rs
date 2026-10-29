// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2004 Jiri "Javky" Javurek (javky@email.cz)
// Derived from JSBSim src/models/propulsion/FGTurboProp.cpp (Calculate's
// phase choice, Off, Run, SpinUp, Start).
//
//! The phases of a turboprop: each updates the engine and returns its
//! power, hp.

use super::{Phase, TurboProp, exp_seek, seek};
use crate::models::propulsion::Inputs;
use crate::models::propulsion::engine::EngineBase;

/// The oil pressure at `n1` and the oil temperature, psi (from MPa).
fn oil_pressure_psi(n1: f64, oil_temp_degk: f64) -> f64 {
    (n1 / 100.0 * 0.25 + (0.1 - (oil_temp_degk - 273.15) * 0.1 / 80.0) * n1 / 100.0) / 7692.0e-6
}

impl TurboProp {
    /// The phase `Calculate` picks before it runs one.
    pub(super) fn select_phase(&mut self, base: &EngineBase, input: &Inputs) {
        let s = &mut self.state;
        // When trimming is finished, the engine is left off or running.
        if s.phase == Phase::Trim && input.total_delta_t > 0.0 {
            if base.running && !base.starved {
                s.phase = Phase::Run;
                s.n1 = self.config.idle_n1;
                s.oil_temp_degk = 366.0;
                s.cutoff = false;
            } else {
                s.phase = Phase::Off;
                s.cutoff = true;
                s.eng_itt_degc = input.tat_c;
                s.eng_temperature = input.tat_c;
                s.oil_temp_degk = input.tat_c + 273.15;
            }
        }
        if !base.running && base.starter && s.phase == Phase::Off {
            s.phase = Phase::SpinUp;
            if s.start_time < 0.0 {
                s.start_time = 0.0;
            }
        }
        if !base.running && !s.cutoff && s.n1 > 15.0 {
            s.phase = Phase::Start;
            s.start_time = -1.0;
        }
        if s.cutoff && s.phase != Phase::SpinUp {
            s.phase = Phase::Off;
        }
        if input.total_delta_t == 0.0 {
            s.phase = Phase::Trim;
        }
        if base.starved {
            s.phase = Phase::Off;
        }
        if s.condition >= 10 {
            s.phase = Phase::Off;
            s.start_time = -1.0;
        }
    }

    /// The power of `EnginePowerRPM_N1` corrected by `power_vc`, at most
    /// the maximum power.
    fn power(&self, power_vc: f64) -> f64 {
        let s = &self.state;
        let mut hp = self.config.engine_power_rpm_n1.value_2d(s.rpm, s.n1);
        hp *= power_vc;
        if hp > self.config.max_power {
            hp = self.config.max_power;
        }
        hp
    }

    /// The ITT goal of a stopped or starting engine: `ITT_N1` at N1, plus
    /// a share of the engine temperature below 20% N1.
    fn cold_itt_goal(&self) -> f64 {
        let s = &self.state;
        let warm = if s.n1 > 20.0 {
            0.0
        } else {
            (20.0 - s.n1) / 20.0 * s.eng_temperature
        };
        self.config.itt_n1.value_2d(s.n1, 0.1) + warm
    }

    /// `Off`: the air turns the generator; friction once the propeller
    /// turns.
    pub(super) fn off(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
        let dt = input.total_delta_t;
        let c = &self.config;
        let s = &mut self.state;
        base.running = false;
        s.eng_starting = false;
        base.fuel_flow_pph = seek(dt, base.fuel_flow_pph, 0.0, 800.0, 800.0);
        s.n1 = exp_seek(
            dt,
            s.n1,
            input.qbar / 15.0,
            c.idle_max_delay * 2.5,
            c.idle_max_delay * 5.0,
        );
        s.oil_temp_degk = exp_seek(dt, s.oil_temp_degk, 273.15 + input.tat_c, 400.0, 400.0);
        s.eng_temperature = exp_seek(dt, s.eng_temperature, input.tat_c, 300.0, 400.0);
        let goal = self.cold_itt_goal();
        let (c, s) = (&self.config, &mut self.state);
        s.eng_itt_degc = exp_seek(dt, s.eng_itt_degc, goal, c.itt_delay, c.itt_delay * 1.2);
        s.oil_pressure_psi = oil_pressure_psi(s.n1, s.oil_temp_degk);
        if s.rpm > 5.0 {
            // Friction in the engine while the propeller turns, an estimate.
            return -0.012;
        }
        0.0
    }

    /// The hot part of `Run` and `Start`: the temperatures follow the ITT
    /// of N1 and its rate of change.
    fn heat(&mut self, old_n1: f64, dt: f64) {
        let s = &mut self.state;
        s.eng_temperature = exp_seek(dt, s.eng_temperature, s.eng_itt_degc, 300.0, 400.0);
        let goal = self
            .config
            .itt_n1
            .value_2d((s.n1 - old_n1) * 300.0 + s.n1, 1.0);
        let delay = self.config.itt_delay;
        s.eng_itt_degc = exp_seek(dt, s.eng_itt_degc, goal, delay, delay * 1.2);
        s.oil_pressure_psi = oil_pressure_psi(s.n1, s.oil_temp_degk);
    }

    /// The fuel flow for `hp` at the combustion efficiency of N1.
    fn fuel_flow(&mut self, base: &mut EngineBase, hp: f64) {
        let s = &mut self.state;
        s.combustion_efficiency = self.config.combustion_efficiency_n1.value_1d(s.n1);
        base.fuel_flow_pph = self.config.psfc / s.combustion_efficiency * hp;
    }

    /// `Run`.
    pub(super) fn run(&mut self, base: &mut EngineBase, power_vc: f64, input: &Inputs) -> f64 {
        let dt = input.total_delta_t;
        base.running = true;
        base.starter = false;
        let (c, s) = (&self.config, &mut self.state);
        s.eng_starting = false;
        let old_n1 = s.n1;
        s.n1 = exp_seek(
            dt,
            s.n1,
            c.idle_n1 + s.throttle_pos * c.n1_factor,
            c.idle_max_delay,
            c.idle_max_delay * 2.4,
        );
        let hp = self.power(power_vc);
        self.fuel_flow(base, hp);
        self.heat(old_n1, dt);
        let s = &mut self.state;
        s.oil_temp_degk = seek(dt, s.oil_temp_degk, 353.15, 0.4 - s.n1 * 0.001, 0.04);
        if s.cutoff || base.starved {
            s.phase = Phase::Off;
        }
        hp
    }

    /// `SpinUp`: the starter turns N1 toward its own speed, if it has
    /// power, until the start times out.
    pub(super) fn spin_up(&mut self, base: &mut EngineBase, power_vc: f64, input: &Inputs) -> f64 {
        let dt = input.total_delta_t;
        let (c, s) = (&self.config, &mut self.state);
        base.running = false;
        s.eng_starting = true;
        base.fuel_flow_pph = 0.0;
        if !s.generator_power {
            s.eng_starting = false;
            s.phase = Phase::Off;
            s.start_time = -1.0;
            return 0.0;
        }
        s.n1 = exp_seek(
            dt,
            s.n1,
            c.starter_n1,
            c.idle_max_delay * 6.0,
            c.idle_max_delay * 2.4,
        );
        s.eng_temperature = exp_seek(dt, s.eng_temperature, input.tat_c, 300.0, 400.0);
        let goal = self.cold_itt_goal();
        let (c, s) = (&self.config, &mut self.state);
        s.eng_itt_degc = exp_seek(dt, s.eng_itt_degc, goal, c.itt_delay, c.itt_delay * 1.2);
        s.oil_temp_degk = exp_seek(dt, s.oil_temp_degk, 273.15 + input.tat_c, 400.0, 400.0);
        s.oil_pressure_psi = oil_pressure_psi(s.n1, s.oil_temp_degk);
        let hp = self.power(power_vc);
        let (c, s) = (&self.config, &mut self.state);
        if s.start_time >= 0.0 {
            s.start_time += dt;
        }
        if s.start_time > c.max_starting_time && c.max_starting_time > 0.0 {
            // The start timed out.
            s.phase = Phase::Off;
            s.start_time = -1.0;
        }
        hp
    }

    /// `Start`: lit at 15% N1 or more, accelerating to idle.
    pub(super) fn start(&mut self, base: &mut EngineBase, power_vc: f64, input: &Inputs) -> f64 {
        let dt = input.total_delta_t;
        let mut hp = 0.0;
        self.state.eng_starting = false;
        if self.state.n1 > 15.0 && !base.starved {
            let old_n1 = self.state.n1;
            base.cranking = true;
            if self.state.n1 < self.config.idle_n1 {
                hp = self.power(power_vc);
                let (c, s) = (&self.config, &mut self.state);
                s.n1 = exp_seek(
                    dt,
                    s.n1,
                    c.idle_n1 * 1.1,
                    c.idle_max_delay * 4.0,
                    c.idle_max_delay * 2.4,
                );
                self.fuel_flow(base, hp);
                self.heat(old_n1, dt);
                let s = &mut self.state;
                s.oil_temp_degk = seek(dt, s.oil_temp_degk, 353.15, 0.4 - s.n1 * 0.001, 0.04);
            } else {
                self.state.phase = Phase::Run;
                base.running = true;
                base.starter = false;
                base.cranking = false;
                base.fuel_flow_pph = 0.0;
            }
        } else {
            self.state.phase = Phase::Off;
            base.starter = false;
        }
        hp
    }
}
