// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGPiston.{h,cpp}.
//
//! The piston engine (`FGPiston`): the throttle and airbox set the
//! manifold pressure, optionally boosted by a multi-speed supercharger;
//! the air and fuel flows follow from the swept volume, the volumetric
//! efficiency and the mixture; the indicated power from the fuel flow,
//! the BSFC and the mixture efficiency, less pumping and friction losses
//! (or, stopped, the starter's torque); and the exhaust gas, cylinder
//! head and oil temperatures and the oil pressure from simple heat
//! balances. The thruster sets the engine speed.

mod bind;
mod cycle;
mod load;
#[cfg(test)]
mod tests;

use super::Inputs;
use super::engine::{EngineBase, EngineType};
use super::families::{EngineFamily, ThrusterSample};
use super::thruster::Thruster;
use crate::Error;
use crate::fdm::{EngineLoad, Models};
use crate::functions::{Evaluator, Table};
use crate::units::{HPTOFTLBSSEC, PSFTOINHG, PSFTOPA, kelvin_to_fahrenheit, rankine_to_kelvin};
use crate::xml::Element;

/// Supercharger speeds an engine may have (`FG_MAX_BOOST_SPEEDS`).
pub const MAX_BOOST_SPEEDS: usize = 3;

/// Gas constant for air, J/kg/K.
const R_AIR: f64 = 287.3;
/// J/kg.
const CALORIFIC_VALUE_FUEL: f64 = 47.3e6;
/// Specific heat at constant pressure, J/kg/K.
const CP_AIR: f64 = 1005.0;
/// J/kg/K.
const CP_FUEL: f64 = 1700.0;
/// Pa.
const STANDARD_PRESSURE: f64 = 101320.73;

/// What the engine file sets once.
#[derive(Debug, Clone)]
pub struct PistonConfig {
    /// Strokes per power stroke.
    pub cycles: f64,
    /// rpm.
    pub idle_rpm: f64,
    /// rpm.
    pub max_rpm: f64,
    /// in^3.
    pub displacement: f64,
    /// m^3.
    pub displacement_si: f64,
    /// hp.
    pub max_hp: f64,
    /// Power kept with a magneto off.
    pub spark_fail_drop: f64,
    /// inHg.
    pub min_manifold_pressure_inhg: f64,
    /// inHg.
    pub max_manifold_pressure_inhg: f64,
    /// Manifold pressure lag, s.
    pub manifold_pressure_lag: f64,
    /// in.
    pub bore: f64,
    /// in.
    pub stroke: f64,
    /// Count.
    pub cylinders: f64,
    /// kg per cylinder head.
    pub cylinder_head_mass: f64,
    /// Compression ratio.
    pub compression_ratio: f64,
    /// Slope of the throttle impedance.
    pub z_throttle: f64,
    /// Mean piston speed where the intake valves choke, ft/s.
    pub peak_mean_piston_speed_fps: f64,
    /// Mean piston speed at `max_rpm`, ft/s.
    pub rated_mean_piston_speed_fps: f64,
    /// Peak starter torque.
    pub starter_torque: f64,
    /// Starter speed where its torque vanishes, rpm.
    pub starter_rpm: f64,
    /// Friction mean effective pressure per mean piston speed, Pa/(m/s).
    pub fmep_dynamic: f64,
    /// Static friction mean effective pressure, Pa.
    pub fmep_static: f64,
    /// Pa.
    pub min_map: f64,
    /// Pa.
    pub max_map: f64,
    /// psi.
    pub oil_press_relief_valve: f64,
    /// rpm.
    pub oil_press_rpm_max: f64,
    /// K.
    pub design_oil_temp: f64,
    /// Oil viscosity index.
    pub oil_viscosity_index: f64,
    /// Combustion efficiency by equivalence ratio.
    pub combustion_efficiency: Table,
    /// Mixture efficiency by fuel/air ratio.
    pub mixture_efficiency: Table,
}

/// What the engine file sets that properties may change.
#[derive(Debug, Clone)]
pub struct PistonTuning {
    /// Power lost to friction, hp.
    pub static_friction_hp: f64,
    /// Indicated specific fuel consumption, lbs/hp/h.
    pub isfc: f64,
    /// Scales the starter torque.
    pub starter_gain: f64,
    /// Volumetric efficiency.
    pub volumetric_efficiency: f64,
    /// Impedance of the air intake before the throttle.
    pub z_airbox: f64,
    /// Share of the ram pressure the intake recovers.
    pub ram_air_factor: f64,
    /// Share of the airspeed that cools the cylinder heads.
    pub cooling_factor: f64,
}

/// The supercharger (`Boost*`, `Rated*`, `Takeoff*` members).
#[derive(Debug, Clone)]
pub struct Boost {
    /// Speeds fitted; 0 for none.
    pub speeds: i32,
    /// Current speed, from 0.
    pub speed: i32,
    /// Supercharged.
    pub boosted: bool,
    /// `<boostmanual>`.
    pub manual: i32,
    /// The pilot selects the speed.
    pub b_manual: bool,
    /// `<boostoverride>`.
    pub override_: i32,
    /// The pilot may override the boost regulator.
    pub b_override: bool,
    /// Takeoff boost above the rated boost is fitted.
    pub b_takeoff: bool,
    /// Sea-level takeoff boost, psi.
    pub takeoff_boost: f64,
    /// Sea-level rated boost by speed, psi.
    pub rated_boost: [f64; MAX_BOOST_SPEEDS],
    /// Altitude where full boost is reached, by speed, ft.
    pub rated_altitude: [f64; MAX_BOOST_SPEEDS],
    /// Speed of the rated power, by speed, rpm.
    pub rated_rpm: [f64; MAX_BOOST_SPEEDS],
    /// Rated power at the rated altitude, by speed, hp.
    pub rated_power: [f64; MAX_BOOST_SPEEDS],
    /// Altitude of the switch to the next speed, ft.
    pub switch_altitude: [f64; MAX_BOOST_SPEEDS - 1],
    /// Pressure of the switch to the next speed, Pa.
    pub switch_pressure: [f64; MAX_BOOST_SPEEDS - 1],
    /// Pressure multiplier of the unregulated supercharger, by speed.
    pub mul: [f64; MAX_BOOST_SPEEDS],
    /// Rated manifold pressure (the regulator's clamp), by speed, Pa.
    pub rated_map: [f64; MAX_BOOST_SPEEDS],
    /// Takeoff manifold pressure clamp, by speed, Pa.
    pub takeoff_map: [f64; MAX_BOOST_SPEEDS],
    /// Hysteresis of the speed switch, Pa.
    pub switch_hysteresis: f64,
    /// Share of the compression work the supercharger takes.
    pub loss_factor: f64,
    /// Power the supercharger takes, hp.
    pub loss_hp: f64,
}

/// The state one `Calculate` updates.
#[derive(Debug, Clone, Default)]
pub struct PistonState {
    /// Ambient pressure, Pa.
    pub p_amb: f64,
    /// Ram pressure at the intake, Pa.
    pub p_ram: f64,
    /// Ambient temperature, K.
    pub t_amb: f64,
    /// Engine speed, rpm.
    pub rpm: f64,
    /// Calibrated airspeed, kt.
    pub ias: f64,
    /// ft/s.
    pub mean_piston_speed_fps: f64,
    /// Manifold pressure, Pa.
    pub map: f64,
    /// Manifold pressure before the supercharger, Pa.
    pub tmap: f64,
    /// Manifold pressure, inHg.
    pub manifold_pressure_inhg: f64,
    /// Pumping mean effective pressure, Pa.
    pub pmep: f64,
    /// Friction mean effective pressure, Pa.
    pub fmep: f64,
    /// hp.
    pub indicated_horse_power: f64,
    /// Shaft power, hp.
    pub hp: f64,
    /// 0 off, 1 left, 2 right, 3 both.
    pub magnetos: i32,
    /// Left magneto on.
    pub magneto_left: bool,
    /// Right magneto on.
    pub magneto_right: bool,
    /// Ambient air density, kg/m^3.
    pub rho_air: f64,
    /// Volumetric efficiency after the pressure ratio.
    pub volumetric_efficiency_reduced: f64,
    /// Air mass flow, kg/s.
    pub m_dot_air: f64,
    /// Air volume flow, m^3/s.
    pub v_dot_air: f64,
    /// Fuel/air over stoichiometric.
    pub equivalence_ratio: f64,
    /// Fuel mass flow, kg/s.
    pub m_dot_fuel: f64,
    /// Combustion efficiency.
    pub combustion_efficiency: f64,
    /// Exhaust gas temperature, K.
    pub exhaust_gas_temp_k: f64,
    /// Exhaust gas temperature at the last reset, °C.
    pub egt_c: f64,
    /// Cylinder head temperature, K.
    pub cylinder_head_temp_k: f64,
    /// psi.
    pub oil_pressure_psi: f64,
    /// K.
    pub oil_temp_k: f64,
}

/// `FGPiston`'s members beyond `FGEngine`'s.
#[derive(Debug, Clone)]
pub struct Piston {
    /// Engine number, for its inputs.
    pub number: usize,
    /// Set once.
    pub config: Box<PistonConfig>,
    /// Set once, changeable through properties.
    pub tuning: PistonTuning,
    /// The supercharger.
    pub boost: Boost,
    /// Updated each step.
    pub state: PistonState,
}

/// Input `i` of `v`, 0 where the vector is short.
fn at(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(0.0)
}

impl Piston {
    /// `FGPiston::ResetToIC` but for `FGEngine`'s: the manifold pressure
    /// at the ambient pressure, the temperatures at the ambient
    /// temperature, the engine and its thruster stopped.
    pub fn reset_state(&mut self, thruster: &mut Thruster, input: &Inputs) {
        let s = &mut self.state;
        s.manifold_pressure_inhg = input.pressure * PSFTOINHG;
        s.map = input.pressure * PSFTOPA;
        s.tmap = s.map;
        let air_temperature_k = rankine_to_kelvin(input.temperature);
        s.oil_temp_k = air_temperature_k;
        s.cylinder_head_temp_k = air_temperature_k;
        s.exhaust_gas_temp_k = air_temperature_k;
        s.egt_c = s.exhaust_gas_temp_k - 273.0;
        thruster.set_rpm(0.0);
        s.rpm = 0.0;
        s.oil_pressure_psi = 0.0;
        self.boost.loss_hp = 0.0;
    }

    /// `getAFR`.
    #[must_use]
    pub fn afr(&self) -> f64 {
        let s = &self.state;
        if s.m_dot_fuel > 0.0 {
            s.m_dot_air / s.m_dot_fuel
        } else {
            f64::INFINITY
        }
    }

    /// `getExhaustGasTemp_degF`.
    #[must_use]
    pub fn exhaust_gas_temp_degf(&self) -> f64 {
        kelvin_to_fahrenheit(self.state.exhaust_gas_temp_k)
    }

    /// `getCylinderHeadTemp_degF`.
    #[must_use]
    pub fn cylinder_head_temp_degf(&self) -> f64 {
        kelvin_to_fahrenheit(self.state.cylinder_head_temp_k)
    }

    /// `getOilTemp_degF`.
    #[must_use]
    pub fn oil_temp_degf(&self) -> f64 {
        kelvin_to_fahrenheit(self.state.oil_temp_k)
    }
}

impl EngineFamily for Piston {
    const TYPE: EngineType = EngineType::Piston;
    type Sample = ThrusterSample;

    fn load(el: &Element, cx: &mut EngineLoad<'_>) -> Result<Self, Error> {
        load::load(el, cx)
    }

    fn sample(
        &self,
        _base: &EngineBase,
        thruster: &Thruster,
        models: &Models,
        ev: &mut Evaluator<'_>,
    ) -> Result<ThrusterSample, Error> {
        thruster.sample(models, ev)
    }

    /// `FGPiston::Calculate` between the pre- and post-functions.
    fn step(
        &mut self,
        base: &mut EngineBase,
        thruster: &mut Thruster,
        sample: ThrusterSample,
        input: &Inputs,
    ) {
        let n = self.number;
        let s = &mut self.state;
        s.p_amb = input.pressure * PSFTOPA;
        let p = input.total_pressure * PSFTOPA;
        s.p_ram = (p - s.p_amb) * self.tuning.ram_air_factor + s.p_amb;
        s.t_amb = rankine_to_kelvin(input.temperature);
        // The thruster sets the engine speed: it holds the gear ratio and
        // the rest of the transmission.
        s.rpm = thruster.engine_rpm();
        s.mean_piston_speed_fps = (s.rpm * self.config.stroke) / 360.0;
        s.ias = input.vc;
        self.do_engine_startup(base);
        if self.boost.boosted {
            self.do_boost_control();
        }
        self.do_map(at(&input.throttle_pos, n), input.total_delta_t);
        self.do_air_flow();
        self.do_fuel_flow(base, at(&input.mixture_pos, n));
        self.do_engine_power(base);
        if self.state.indicated_horse_power < 0.1250 {
            base.running = false;
        }
        self.do_egt(base, input);
        self.do_cht(input.total_delta_t);
        self.do_oil_temperature(input.total_delta_t);
        self.do_oil_pressure();
        thruster.set_advance(at(&input.prop_advance, n));
        thruster.set_feather(input.prop_feather.get(n).copied().unwrap_or(false));
        thruster.base.input.load(input);
        let mut power = self.state.hp * HPTOFTLBSSEC;
        // No negative power while the propeller is not turning.
        if self.state.rpm <= 0.1 && power < 0.0 {
            power = 0.0;
        }
        thruster.calculate(power, &sample);
    }

    /// `FGPiston::CalcFuelNeed`: the fuel flow the cycle computed.
    fn calc_fuel_need(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
        base.fuel_expended = base.fuel_flow_rate * input.total_delta_t;
        if !base.starved {
            base.fuel_used_lbs += base.fuel_expended;
        }
        base.fuel_expended
    }

    /// `FGPiston::InitRunning`: both magnetos, the mixture for the
    /// ambient pressure and the propeller at twice the idle speed.
    fn init_running(
        &mut self,
        base: &mut EngineBase,
        thruster: &mut Thruster,
        input: &mut Inputs,
    ) -> i32 {
        let n = self.number;
        self.state.magnetos = 3;
        let mixture = input.pressure_ratio * 1.3;
        if let Some(m) = input.mixture_cmd.get_mut(n) {
            *m = mixture;
        }
        if let Some(m) = input.mixture_pos.get_mut(n) {
            *m = mixture;
        }
        let rpm = 2.0 * self.config.idle_rpm / thruster.base.gear_ratio;
        thruster.set_rpm(rpm);
        base.running = true;
        1
    }

    /// `FGPiston::ResetToIC`.
    fn reset_to_ic(&mut self, base: &mut EngineBase, thruster: &mut Thruster, input: &Inputs) {
        base.reset_to_ic();
        thruster.reset_to_ic();
        self.reset_state(thruster, input);
    }

    fn set_magnetos(&mut self, setting: i32) {
        self.state.magnetos = setting;
    }
}
