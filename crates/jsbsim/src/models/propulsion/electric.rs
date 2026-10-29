// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2004 David Culp (davidculp2@comcast.net)
// Derived from JSBSim src/models/propulsion/FGElectric.{h,cpp}.
//
//! The electric engine (`FGElectric`): its power is its rated power times
//! the throttle, and it burns no fuel.

#[cfg(test)]
mod tests;

use super::Inputs;
use super::engine::{EngineBase, EngineType};
use super::families::{EngineFamily, EngineKind, ThrusterSample};
use super::thruster::Thruster;
use crate::Error;
use crate::fdm::{Binder, EngineLoad, Kind, ModelEvaluator, Models};
use crate::units::HPTOFTLBSSEC;
use crate::xml::Element;

/// Watts per horsepower, as `FGElectric` holds it.
const HP_TO_WATTS: f64 = 745.7;

/// `FGElectric`'s members beyond `FGEngine`'s.
#[derive(Debug, Clone)]
pub struct Electric {
    /// Engine number, for its inputs.
    pub number: usize,
    /// Rated power, W.
    pub power_watts: f64,
    /// Shaft speed, rpm.
    pub rpm: f64,
    /// Output, hp.
    pub hp: f64,
}

fn electric(m: &Models, n: usize) -> Option<&Electric> {
    match &m.propulsion.engines.get(n)?.kind {
        EngineKind::Electric(e) => Some(e),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

/// The constructor's tie of `propulsion/engine[n]/power-hp` to `HP`.
fn bind(b: &mut Binder<'_>, n: usize) -> Result<(), Error> {
    b.property(
        &format!("propulsion/engine[{n}]/power-hp"),
        Kind::Double,
        move |m| electric(m, n).map_or(0.0, |e| e.hp),
        move |m, v| {
            if let Some(EngineKind::Electric(e)) =
                m.propulsion.engines.get_mut(n).map(|e| &mut e.kind)
            {
                e.hp = v;
            }
        },
    )?;
    Ok(())
}

impl EngineFamily for Electric {
    const TYPE: EngineType = EngineType::Electric;
    type Sample = ThrusterSample;

    /// `FGElectric(exec, el, n, input)`: `FGEngine::Load`, then the rated
    /// power (1 hp unless `<power>` sets it).
    fn load(el: &Element, cx: &mut EngineLoad<'_>) -> Result<Self, Error> {
        cx.load_base(el, EngineType::Unknown)?;
        let power_watts = match el.find_element("power") {
            Some(_) => el.find_element_value_as_number_convert_to("power", "WATTS")?,
            None => HP_TO_WATTS,
        };
        let n = cx.number();
        bind(&mut cx.context().binder, n)?;
        Ok(Self {
            number: n,
            power_watts,
            rpm: 0.0,
            hp: 0.0,
        })
    }

    fn sample(
        &self,
        _base: &EngineBase,
        thruster: &Thruster,
        models: &Models,
        ev: &mut ModelEvaluator<'_>,
    ) -> Result<ThrusterSample, Error> {
        thruster.sample(models, ev)
    }

    /// `FGElectric::Calculate` between the pre- and post-functions.
    fn step(
        &mut self,
        _base: &mut EngineBase,
        thruster: &mut Thruster,
        sample: ThrusterSample,
        input: &Inputs,
    ) {
        let n = self.number;
        if thruster.is_propeller() {
            thruster.set_advance(input.prop_advance.get(n).copied().unwrap_or(0.0));
            thruster.set_feather(input.prop_feather.get(n).copied().unwrap_or(false));
        }
        self.rpm = thruster.rpm() * thruster.base.gear_ratio;
        let throttle = input.throttle_pos.get(n).copied().unwrap_or(0.0);
        self.hp = self.power_watts * throttle / HP_TO_WATTS;
        thruster.base.input.load(input);
        let mut power = self.hp * HPTOFTLBSSEC;
        // No negative power while the propeller is not turning:
        // `std::max(power, 0.0)`.
        if self.rpm <= 0.1 && power < 0.0 {
            power = 0.0;
        }
        thruster.calculate(power, &sample);
    }

    /// `FGElectric::CalcFuelNeed`: none, and nothing burnt is recorded.
    fn calc_fuel_need(&mut self, _base: &mut EngineBase, _input: &Inputs) -> f64 {
        0.0
    }
}

crate::snapshot::visit_fields! { Electric { number, power_watts, rpm, hp } }
