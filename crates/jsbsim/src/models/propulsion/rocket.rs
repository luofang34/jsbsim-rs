// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGRocket.{h,cpp}.
//
//! The rocket engine (`FGRocket`), liquid or solid, through its nozzle.
//!
//! A liquid rocket's vacuum thrust is its specific impulse times the
//! propellant it burnt over the last step, and its fuel and oxidizer flows
//! are the maximum propellant flow split by the mixture ratio, times the
//! throttle; below the minimum throttle, or starved, it flames out. A
//! solid rocket (one with a `<thrust_table>`) ignites at full throttle and
//! then burns on regardless of it, its vacuum thrust looked up by the
//! propellant burnt, scaled by the thrust and total-impulse variations and
//! ramped up by a quarter sine over the build-up time; its fuel flow is
//! what that thrust burns at its specific impulse. While integration is
//! suspended it does nothing at all.

mod bind;
#[cfg(test)]
mod tests;

use std::f64::consts::PI;

use super::Inputs;
use super::engine::{EngineBase, EngineType};
use super::families::{EngineFamily, ThrusterSample};
use super::thruster::Thruster;
use crate::Error;
use crate::fdm::{EngineLoad, ModelEvaluator, Models};
use crate::functions::{FnId, Table};
use crate::xml::Element;

/// `FGRocket`'s members beyond `FGEngine`'s.
#[derive(Debug, Clone)]
pub struct Rocket {
    /// Engine number, for its inputs.
    pub number: usize,
    /// Vacuum specific impulse, s.
    pub isp: f64,
    /// `<isp>`'s `<function>`, which sets `isp` before each thrust.
    pub isp_function: Option<FnId>,
    /// Total impulse delivered, lbs·s.
    pub it: f64,
    /// Total vacuum impulse, lbs·s.
    pub it_vac: f64,
    /// Oxidizer to fuel mixture ratio.
    pub mxr: f64,
    /// Time burnt since ignition (solid), s.
    pub burn_time: f64,
    /// Thrust variation of a solid rocket, as a fraction.
    pub thrust_variation: f64,
    /// Total impulse variation of a solid rocket, as a fraction.
    pub total_isp_variation: f64,
    /// Vacuum thrust, lbs.
    pub vac_thrust: f64,
    /// Oxidizer burnt in the last step, lbs.
    pub oxidizer_expended: f64,
    /// Propellant burnt since loading, lbs.
    pub total_propellant_expended: f64,
    /// Sea-level oxidizer flow at full throttle, lbs/s.
    pub sl_oxi_flow_max: f64,
    /// Sea-level propellant flow at full throttle, lbs/s.
    pub prop_flow_max: f64,
    /// Oxidizer flow, lbs/s.
    pub oxidizer_flow_rate: f64,
    /// Propellant flow of the last step, lbs/s.
    pub propellant_flow_rate: f64,
    /// Below the minimum throttle or starved (liquid).
    pub flameout: bool,
    /// Time to full thrust after ignition (solid), s.
    pub buildup_time: f64,
    /// Vacuum thrust by propellant burnt: present for a solid rocket.
    pub thrust_table: Option<Box<Table>>,
}

/// The optional number `name` of `el`.
fn number(el: &Element, name: &str) -> Result<Option<f64>, Error> {
    match el.find_element(name) {
        Some(_) => Ok(Some(el.find_element_value_as_number(name)?)),
        None => Ok(None),
    }
}

/// The optional number `name` of `el` in `unit`.
fn flow(el: &Element, name: &str) -> Result<Option<f64>, Error> {
    match el.find_element(name) {
        Some(_) => Ok(Some(
            el.find_element_value_as_number_convert_to(name, "LBS/SEC")?,
        )),
        None => Ok(None),
    }
}

impl Rocket {
    /// `FGRocket`'s member initializers and the constructor's defaults.
    fn new(number: usize) -> Self {
        Self {
            number,
            isp: 0.0,
            isp_function: None,
            it: 0.0,
            it_vac: 0.0,
            mxr: 0.0,
            burn_time: 0.0,
            thrust_variation: 0.0,
            total_isp_variation: 0.0,
            vac_thrust: 0.0,
            oxidizer_expended: 0.0,
            total_propellant_expended: 0.0,
            sl_oxi_flow_max: 0.0,
            prop_flow_max: 0.0,
            oxidizer_flow_rate: 0.0,
            propellant_flow_rate: 0.0,
            flameout: false,
            buildup_time: 0.0,
            thrust_table: None,
        }
    }

    /// The specific impulse, flow and throttle parameters of the
    /// constructor; the isp function, when `<isp>` holds one.
    fn read(&mut self, el: &Element, cx: &mut EngineLoad<'_>) -> Result<(), Error> {
        let Some(isp_el) = el.find_element("isp") else {
            return Err(Error::Model {
                message: format!(
                    "{}: Specific Impulse <isp> must be specified for a rocket engine",
                    el.location()
                ),
            });
        };
        match isp_el.find_element("function") {
            Some(f) => {
                let prefix = self.number.to_string();
                let ctx = cx.context();
                let id = ctx.functions.load_function(ctx.binder.tree, f, &prefix)?;
                self.isp_function = Some(id);
            }
            None => self.isp = el.find_element_value_as_number("isp")?,
        }
        if let Some(t) = number(el, "builduptime")? {
            self.buildup_time = t;
        }
        let base = cx.base_mut().ok_or_else(|| Error::Model {
            message: format!("{}: the rocket did not load its base", el.location()),
        })?;
        base.min_throttle = 0.0;
        base.max_throttle = 1.0;
        if let Some(t) = number(el, "maxthrottle")? {
            base.max_throttle = t;
        }
        if let Some(t) = number(el, "minthrottle")? {
            base.min_throttle = t;
        }
        if let Some(fuel) = flow(el, "slfuelflowmax")? {
            base.sl_fuel_flow_max = fuel;
            if let Some(oxi) = flow(el, "sloxiflowmax")? {
                self.sl_oxi_flow_max = oxi;
            }
            self.prop_flow_max = self.sl_oxi_flow_max + fuel;
            self.mxr = self.sl_oxi_flow_max / fuel;
        } else if let Some(prop) = flow(el, "propflowmax")? {
            self.prop_flow_max = prop;
            if let Some(mxr) = number(el, "mixtureratio")? {
                self.mxr = mxr;
            }
        }
        Ok(())
    }

    /// The `<thrust_table>` of a solid rocket and its `<variation>`.
    fn read_solid(&mut self, el: &Element, cx: &mut EngineLoad<'_>) -> Result<(), Error> {
        let Some(table_el) = el.find_element("thrust_table") else {
            return Ok(());
        };
        let ctx = cx.context();
        let id = ctx.functions.load_table(ctx.binder.tree, table_el, "")?;
        self.thrust_table = Some(Box::new(ctx.functions.table(id)?.clone()));
        if let Some(variation) = el.find_element("variation") {
            if let Some(v) = number(variation, "thrust")? {
                self.thrust_variation = v;
            }
            if let Some(v) = number(variation, "total_isp")? {
                self.total_isp_variation = v;
            }
        }
        Ok(())
    }
}

/// What a rocket's `Calculate` reads: the isp function's value and its
/// nozzle's reads.
#[derive(Debug)]
pub struct RocketSample {
    /// The isp function's value, when the rocket has one.
    pub isp: Option<f64>,
    /// The thruster's reads.
    pub thruster: ThrusterSample,
}

impl EngineFamily for Rocket {
    const TYPE: EngineType = EngineType::Rocket;
    type Sample = RocketSample;
    const IDLE_WHILE_SUSPENDED: bool = true;

    /// `FGRocket(exec, el, n, input)`: `FGEngine::Load`, the ties, then
    /// the parameters, the isp function's first value and, for a solid
    /// rocket, its thrust table. The ties are made before the table is
    /// read, so every rocket gets the liquid rocket's (JSBSim's
    /// `bindmodel` tests the table before it is loaded).
    fn load(el: &Element, cx: &mut EngineLoad<'_>) -> Result<Self, Error> {
        cx.load_base(el, EngineType::Unknown)?;
        let n = cx.number();
        let mut rocket = Self::new(n);
        bind::bind(&mut cx.context().binder, n)?;
        rocket.read(el, cx)?;
        if let Some(id) = rocket.isp_function {
            rocket.isp = cx.function_value(id)?;
        }
        rocket.read_solid(el, cx)?;
        Ok(rocket)
    }

    fn sample(
        &self,
        _base: &EngineBase,
        thruster: &Thruster,
        models: &Models,
        ev: &mut ModelEvaluator<'_>,
    ) -> Result<RocketSample, Error> {
        let isp = match self.isp_function {
            Some(id) => Some(ev.function(id)?),
            None => None,
        };
        Ok(RocketSample {
            isp,
            thruster: thruster.sample(models, ev)?,
        })
    }

    /// `FGRocket::Calculate` between the pre- and post-functions.
    fn step(
        &mut self,
        base: &mut EngineBase,
        thruster: &mut Thruster,
        sample: RocketSample,
        input: &Inputs,
    ) {
        let dt = input.total_delta_t;
        let burnt = base.fuel_expended + self.oxidizer_expended;
        self.propellant_flow_rate = burnt / dt;
        self.total_propellant_expended += burnt;
        if let Some(isp) = sample.isp {
            self.isp = isp;
        }
        let throttle = input.throttle_pos.get(self.number).copied().unwrap_or(0.0);
        if let Some(table) = &self.thrust_table {
            if (throttle == 1.0 || self.burn_time > 0.0) && !base.starved {
                self.vac_thrust = table.value_1d(self.total_propellant_expended)
                    * (self.thrust_variation + 1.0)
                    * (self.total_isp_variation + 1.0);
                if self.burn_time <= self.buildup_time && self.buildup_time > 0.0 {
                    self.vac_thrust *= libm::sin((self.burn_time / self.buildup_time) * PI / 2.0);
                }
                self.burn_time += dt;
            } else {
                self.vac_thrust = 0.0;
            }
        } else if throttle < base.min_throttle || base.starved {
            base.pct_power = 0.0;
            self.flameout = true;
            self.vac_thrust = 0.0;
        } else {
            base.pct_power = throttle;
            self.flameout = false;
            self.vac_thrust = self.isp * self.propellant_flow_rate;
        }
        thruster.base.input.load(input);
        self.it += thruster.calculate(self.vac_thrust, &sample.thruster) * dt;
        self.it_vac += self.vac_thrust * dt;
    }

    /// `FGRocket::CalcFuelNeed`: a solid rocket burns what its thrust
    /// needs at its specific impulse (less the impulse variation), a
    /// liquid one its share of the propellant flow at its power. Unlike
    /// `FGEngine`'s, it does not add to the fuel used.
    fn calc_fuel_need(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
        if self.thrust_table.is_some() {
            base.fuel_flow_rate = self.vac_thrust / self.isp;
            base.fuel_flow_rate /= 1.0 + self.total_isp_variation;
        } else {
            base.sl_fuel_flow_max = self.prop_flow_max / (1.0 + self.mxr);
            base.fuel_flow_rate = base.sl_fuel_flow_max * base.pct_power;
        }
        base.fuel_expended = base.fuel_flow_rate * input.total_delta_t;
        base.fuel_expended
    }

    /// `FGRocket::CalcOxidizerNeed`: the oxidizer's share of the
    /// propellant flow at the current power.
    fn calc_oxidizer_need(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
        self.sl_oxi_flow_max = self.prop_flow_max * self.mxr / (1.0 + self.mxr);
        self.oxidizer_flow_rate = self.sl_oxi_flow_max * base.pct_power;
        self.oxidizer_expended = self.oxidizer_flow_rate * input.total_delta_t;
        self.oxidizer_expended
    }
}
