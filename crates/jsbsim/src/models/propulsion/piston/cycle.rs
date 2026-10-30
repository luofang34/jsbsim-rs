// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGPiston.cpp (doEngineStartup
// to doOilPressure, GetStdPressure100K).
//
//! One step of the piston engine's cycle, in `Calculate`'s order.

use super::{CALORIFIC_VALUE_FUEL, CP_AIR, CP_FUEL, Piston, R_AIR};
use crate::models::propulsion::Inputs;
use crate::models::propulsion::engine::EngineBase;
use crate::units::{FTTOM, INHGTOPA, rankine_to_kelvin};

/// Entry `i` of `v`, NaN past its end (where JSBSim would read past the
/// array).
fn entry<const N: usize>(v: &[f64; N], i: i32) -> f64 {
    usize::try_from(i)
        .ok()
        .and_then(|i| v.get(i))
        .copied()
        .unwrap_or(f64::NAN)
}

/// `GetStdPressure100K`: FGStandardAtmosphere's polynomial for the
/// pressure at `altitude` ft (limited to 100000 ft), psf.
#[must_use]
pub fn std_pressure_100k(altitude: f64) -> f64 {
    const COEF: [f64; 5] = [
        2116.217,
        -7.648932746E-2,
        1.0925498604E-6,
        -7.1135726027E-12,
        1.7470331356E-17,
    ];
    let altitude = if altitude > 100000.0 {
        100000.0
    } else {
        altitude
    };
    // The powers of the altitude, each the previous times the altitude.
    let mut power = 1.0;
    let mut press = 0.0;
    for c in COEF {
        press += c * power;
        power *= altitude;
    }
    press
}

impl Piston {
    /// `doEngineStartup`: the magnetos, the starter, and whether the
    /// engine runs (it starts, windmilling too, above 80% of idle speed
    /// with spark and fuel).
    pub(super) fn do_engine_startup(&mut self, base: &mut EngineBase) {
        let s = &mut self.state;
        s.magneto_left = false;
        s.magneto_right = false;
        // Battery voltage and the master switch are neglected.
        let spark = s.magnetos != 0;
        if s.magnetos == 1 || s.magnetos > 2 {
            s.magneto_left = true;
        }
        if s.magnetos > 1 {
            s.magneto_right = true;
        }
        // Any fuel flow runs it; too little shows in the power.
        let fuel = base.fuel_flow_rate > 0.0;
        if base.cranking != base.starter {
            base.cranking = base.starter;
        }
        let idle = self.config.idle_rpm;
        if base.running {
            if !spark || !fuel {
                base.running = false;
            }
            if s.rpm < idle * 0.8 {
                base.running = false;
            }
        } else if spark && fuel && s.rpm > idle * 0.8 {
            base.running = true;
        }
    }

    /// `doBoostControl`: the supercharger speed, manual or switched by
    /// the ambient pressure with hysteresis.
    pub(super) fn do_boost_control(&mut self) {
        let b = &mut self.boost;
        if b.b_manual {
            if b.speed > b.speeds - 1 {
                b.speed = b.speeds - 1;
            }
            if b.speed < 0 {
                b.speed = 0;
            }
            return;
        }
        let p_amb = self.state.p_amb;
        if b.speed < b.speeds - 1
            && p_amb < entry(&b.switch_pressure, b.speed) - b.switch_hysteresis
        {
            b.speed += 1;
        }
        if b.speed > 0 && p_amb > entry(&b.switch_pressure, b.speed - 1) + b.switch_hysteresis {
            b.speed -= 1;
        }
    }

    /// `doMAP`: the manifold pressure from the throttle, the intake and
    /// engine impedances (lagged), the supercharger and its regulator,
    /// and the supercharger's power.
    pub(super) fn do_map(&mut self, throttle: f64, dt: f64) {
        let (c, t, b, s) = (&self.config, &self.tuning, &mut self.boost, &mut self.state);
        let zt = (1.0 - throttle) * (1.0 - throttle) * c.z_throttle;
        let ze = if s.mean_piston_speed_fps > 0.0 {
            c.peak_mean_piston_speed_fps / s.mean_piston_speed_fps
        } else {
            999999.0
        };
        let map_coefficient = ze / (ze + t.z_airbox + zt);
        let mut d_map = s.tmap - s.p_ram * map_coefficient;
        if c.manifold_pressure_lag > dt {
            d_map *= dt / c.manifold_pressure_lag;
        }
        s.tmap -= d_map;
        // p_amb stands in for the exhaust manifold pressure.
        s.pmep = (s.tmap - s.p_amb) * t.volumetric_efficiency;
        if b.boosted {
            // Above 98% throttle is the takeoff boost, where fitted.
            let takeoff_pos = b.b_takeoff && throttle > 0.98;
            let speed = b.speed;
            let boost_factor =
                ((entry(&b.mul, speed) - 1.0) / entry(&b.rated_rpm, speed)) * s.rpm + 1.0;
            s.map = s.tmap * boost_factor;
            if !b.b_override {
                let clamp = if takeoff_pos {
                    entry(&b.takeoff_map, speed)
                } else {
                    entry(&b.rated_map, speed)
                };
                if s.map > clamp {
                    s.map = clamp;
                }
            }
        } else {
            s.map = s.tmap;
        }
        if b.loss_factor > 0.0 {
            let gamma = 1.414;
            let n_stage = 1.0;
            b.loss_hp = ((n_stage * s.tmap * s.v_dot_air * gamma) / (gamma - 1.0))
                * (crate::elementary::pow(s.map / s.tmap, (gamma - 1.0) / (n_stage * gamma)) - 1.0)
                * b.loss_factor
                / 745.7;
        } else {
            b.loss_hp = 0.0;
        }
        s.manifold_pressure_inhg = s.map / INHGTOPA;
    }

    /// `doAirFlow`: the air flow through the engine, with the volumetric
    /// efficiency reduced by the manifold to exhaust pressure ratio
    /// (Taylor, The Internal Combustion Engine, eq. 6-10).
    pub(super) fn do_air_flow(&mut self) {
        let (c, t, s) = (&self.config, &self.tuning, &mut self.state);
        let gamma = 1.3;
        let cr = c.compression_ratio;
        let mut mratio = if s.map < 1.0 { cr } else { s.p_amb / s.map };
        if mratio > cr {
            mratio = cr;
        }
        let ve = ((gamma - 1.0) / gamma) + (cr - mratio) / (gamma * (cr - 1.0));
        s.rho_air = s.p_amb / (R_AIR * s.t_amb);
        let swept_volume = (c.displacement_si * (s.rpm / 60.0)) / 2.0;
        s.volumetric_efficiency_reduced = t.volumetric_efficiency * ve;
        s.v_dot_air = swept_volume * s.volumetric_efficiency_reduced;
        let rho_air_manifold = s.map / (R_AIR * s.t_amb);
        s.m_dot_air = s.v_dot_air * rho_air_manifold;
    }

    /// `doFuelFlow`: the fuel flow the mixture meters into the air flow.
    pub(super) fn do_fuel_flow(&mut self, base: &mut EngineBase, mixture: f64) {
        let s = &mut self.state;
        // An air/fuel ratio from infinity to 11.3075.
        let thi_sea_level = 1.3 * mixture;
        s.equivalence_ratio = thi_sea_level * 101325.0 / s.p_amb;
        s.m_dot_fuel = (s.m_dot_air * s.equivalence_ratio) / 14.7;
        base.fuel_flow_rate = s.m_dot_fuel * 2.2046;
        if base.starved {
            s.equivalence_ratio = 0.0;
            base.fuel_flow_rate = 0.0;
            s.m_dot_fuel = 0.0;
        }
        base.fuel_flow_pph = base.fuel_flow_rate * 3600.0;
        base.fuel_flow_gph = base.fuel_flow_pph / base.fuel_density;
    }

    /// `doEnginePower`: the indicated power of combustion (or of the
    /// starter), the pumping and friction losses (Lumley, Engines: An
    /// Introduction, fig. 4.4) and the supercharger's power.
    pub(super) fn do_engine_power(&mut self, base: &mut EngineBase) {
        let (c, t, s) = (&self.config, &self.tuning, &mut self.state);
        s.indicated_horse_power = -t.static_friction_hp;
        s.fmep = 0.0;
        if base.running {
            let me = c.mixture_efficiency.value_1d(s.m_dot_fuel / s.m_dot_air);
            s.fmep = -c.fmep_dynamic * s.mean_piston_speed_fps * FTTOM - c.fmep_static;
            let mut power = 1.0;
            if s.magnetos != 3 {
                power *= c.spark_fail_drop;
            }
            s.indicated_horse_power =
                (base.fuel_flow_pph / t.isfc) * me * power - t.static_friction_hp;
        } else {
            let rpm = if s.rpm < 1.0 { 1.0 } else { s.rpm };
            if base.cranking {
                let k_torque = if s.rpm < c.starter_rpm {
                    1.0 - s.rpm / c.starter_rpm
                } else {
                    0.0
                };
                let torque = c.starter_torque * k_torque * t.starter_gain;
                s.indicated_horse_power = torque * rpm / 5252.0;
            }
        }
        // (1/2) for the cycles, 60 for minutes and 745.7 W per hp.
        let pumping_hp = ((s.pmep + s.fmep) * c.displacement_si * s.rpm) / (c.cycles * 22371.0);
        s.hp = s.indicated_horse_power + pumping_hp - self.boost.loss_hp;
        base.pct_power = s.hp / c.max_hp;
    }

    /// `doEGT`: the exhaust gas temperature from the combustion's energy
    /// balance, or falling to ambient.
    pub(super) fn do_egt(&mut self, base: &EngineBase, input: &Inputs) {
        let (c, s) = (&self.config, &mut self.state);
        if base.running && s.m_dot_air > 0.0 {
            s.combustion_efficiency = c.combustion_efficiency.value_1d(s.equivalence_ratio);
            let enthalpy_exhaust =
                s.m_dot_fuel * CALORIFIC_VALUE_FUEL * s.combustion_efficiency * 0.30;
            let heat_capacity_exhaust = (CP_AIR * s.m_dot_air) + (CP_FUEL * s.m_dot_fuel);
            let delta_t_exhaust = enthalpy_exhaust / heat_capacity_exhaust;
            s.exhaust_gas_temp_k = s.t_amb + delta_t_exhaust;
        } else {
            // A guessed time constant.
            s.combustion_efficiency = 0.0;
            let d_egt_dt = (rankine_to_kelvin(input.temperature) - s.exhaust_gas_temp_k) / 100.0;
            let delta_t_exhaust = d_egt_dt * input.total_delta_t;
            s.exhaust_gas_temp_k += delta_t_exhaust;
        }
    }

    /// `doCHT`: the cylinder head temperature from combustion heating and
    /// forced and free cooling.
    pub(super) fn do_cht(&mut self, dt: f64) {
        let (c, t, s) = (&self.config, &self.tuning, &mut self.state);
        let h1 = -95.0;
        let h2 = -3.95;
        // -0.05 times the default maximum speed, 2800 rpm.
        let h3 = -140.0;
        let arbitary_area = c.displacement / 360.0;
        let cp_cylinder_head = 800.0;
        let mass_cylinder_head = c.cylinder_head_mass * c.cylinders;
        let temperature_difference = s.cylinder_head_temp_k - s.t_amb;
        let v_apparent = s.ias * t.cooling_factor;
        let v_dot_cooling_air = arbitary_area * v_apparent;
        let m_dot_cooling_air = v_dot_cooling_air * s.rho_air;
        let dqdt_from_combustion =
            s.m_dot_fuel * CALORIFIC_VALUE_FUEL * s.combustion_efficiency * 0.33;
        let dqdt_forced = (h2 * m_dot_cooling_air * temperature_difference)
            + (h3 * s.rpm * temperature_difference / c.max_rpm);
        let dqdt_free = h1 * temperature_difference * arbitary_area;
        let dqdt_cylinder_head = dqdt_from_combustion + dqdt_forced + dqdt_free;
        let heat_capacity_cylinder_head = cp_cylinder_head * mass_cylinder_head;
        s.cylinder_head_temp_k += (dqdt_cylinder_head / heat_capacity_cylinder_head) * dt;
    }

    /// `doOilTemperature`: toward a target between the cylinder head and
    /// ambient temperatures, faster with more oil pressure.
    pub(super) fn do_oil_temperature(&mut self, dt: f64) {
        let s = &mut self.state;
        let efficiency = 0.667;
        let target_oil_temp =
            s.cylinder_head_temp_k + efficiency * (s.t_amb - s.cylinder_head_temp_k);
        // About 80 s at 60 psi; the oil does not circulate below 5 psi.
        let time_constant = if s.oil_pressure_psi > 5.0 {
            5000.0 / s.oil_pressure_psi
        } else {
            1000.0
        };
        let d_oil_temp_dt = (target_oil_temp - s.oil_temp_k) / time_constant;
        s.oil_temp_k += d_oil_temp_dt * dt;
    }

    /// `doOilPressure`: with the speed up to the relief valve, corrected
    /// for the oil's viscosity at its temperature.
    pub(super) fn do_oil_pressure(&mut self) {
        let (c, s) = (&self.config, &mut self.state);
        s.oil_pressure_psi = (c.oil_press_relief_valve / c.oil_press_rpm_max) * s.rpm;
        if s.oil_pressure_psi >= c.oil_press_relief_valve {
            s.oil_pressure_psi = c.oil_press_relief_valve;
        }
        s.oil_pressure_psi +=
            (c.design_oil_temp - s.oil_temp_k) * c.oil_viscosity_index * s.oil_pressure_psi
                / c.oil_press_relief_valve;
    }
}
