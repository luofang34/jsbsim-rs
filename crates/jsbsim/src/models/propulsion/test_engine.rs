// SPDX-License-Identifier: LGPL-2.1-or-later
//! A rocket and a direct thruster registered only by the tests, so the
//! propulsion framework (loading, fuel feeding, the force sums, the
//! `propulsion/` properties) runs through the dispatch an engine type
//! will use.
//!
//! `<test_engine>` reads `<sl-fuel-flow>` (lbs/s at full power); while
//! running it burns at full power, and half as much oxidizer, and its
//! thruster pushes along the body x axis with 1000 lbs times the power.

use super::Inputs;
use super::engine::{EngineBase, EngineType};
use super::families::{EngineFamily, InitRunning, ThrusterFamily, ThrusterSite};
use super::thruster::{Thruster, ThrusterBase};
use crate::Error;
use crate::fdm::ModelEvaluator;
use crate::fdm::{EngineLoad, LoadContext, Models};
use crate::math::{ColumnVector3, E_X};
use crate::xml::Element;

/// The test rocket.
#[derive(Debug, Clone)]
pub struct TestEngine {
    /// Calculations run.
    pub calculations: u32,
    /// Throttle position read by the last sample.
    pub throttle: f64,
}

impl EngineFamily for TestEngine {
    const TYPE: EngineType = EngineType::Rocket;
    type Sample = (f64, super::families::ThrusterSample);

    fn load(el: &Element, cx: &mut EngineLoad<'_>) -> Result<Self, Error> {
        cx.load_base(el, Self::TYPE)?;
        let flow = el.find_element_value_as_number("sl-fuel-flow")?;
        if let Some(base) = cx.base_mut() {
            base.sl_fuel_flow_max = flow;
        }
        Ok(Self {
            calculations: 0,
            throttle: 0.0,
        })
    }

    fn sample(
        &self,
        base: &EngineBase,
        thruster: &Thruster,
        models: &Models,
        ev: &mut ModelEvaluator<'_>,
    ) -> Result<Self::Sample, Error> {
        let throttle = models
            .propulsion
            .input
            .throttle_pos
            .get(base.number)
            .copied()
            .unwrap_or(0.0);
        Ok((throttle, thruster.sample(models, ev)?))
    }

    fn step(
        &mut self,
        base: &mut EngineBase,
        thruster: &mut Thruster,
        sample: Self::Sample,
        input: &Inputs,
    ) {
        self.calculations = self.calculations.wrapping_add(1);
        self.throttle = sample.0;
        base.pct_power = if base.running && !base.starved {
            1.0
        } else {
            0.0
        };
        thruster.base.input.load(input);
        thruster.calculate(1000.0 * base.pct_power, &sample.1);
    }

    fn calc_oxidizer_need(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
        0.5 * base.sl_fuel_flow_max * base.pct_power * input.total_delta_t
    }

    fn init_running(
        &mut self,
        base: &mut EngineBase,
        _thruster: &mut Thruster,
        _input: &mut Inputs,
    ) -> InitRunning {
        base.running = true;
        InitRunning::Done
    }
}

/// The test thruster.
#[derive(Debug, Clone)]
pub struct TestThruster;

impl ThrusterFamily for TestThruster {
    type Sample = ();

    fn load(
        el: &Element,
        _thruster_el: &Element,
        base: &mut ThrusterBase,
        _ctx: &mut LoadContext<'_>,
        _site: &ThrusterSite<'_>,
    ) -> Result<Self, Error> {
        base.name = el.attribute("name").to_owned();
        base.force.set_location(ColumnVector3::new(10.0, 0.0, 12.0));
        Ok(Self)
    }

    fn sample(
        &self,
        _base: &ThrusterBase,
        _models: &Models,
        _ev: &mut ModelEvaluator<'_>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn calculate(&mut self, base: &mut ThrusterBase, power: f64, _sample: &()) -> f64 {
        base.thrust = power;
        base.force.native_force[E_X] = power;
        power
    }
}
