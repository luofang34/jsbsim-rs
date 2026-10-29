// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2022 Paolo Becchi (pbecchi@aerobusinees.it)
// Derived from JSBSim src/models/propulsion/FGBrushLessDCMotor.{h,cpp}.
//
//! The brushless DC motor (`FGBrushLessDCMotor`): Drela's first-order
//! model, in which the throttle sets the voltage, the back EMF of the
//! propeller's speed opposes it across the coil resistance, and the
//! no-load current is the friction. It burns no fuel.

#[cfg(test)]
mod tests;

use std::f64::consts::PI;

use super::Inputs;
use super::engine::{EngineBase, EngineType};
use super::families::{EngineFamily, EngineKind, ThrusterSample};
use super::thruster::Thruster;
use crate::Error;
use crate::fdm::{Binder, EngineLoad, Kind, ModelEvaluator, Models};
use crate::units::constrain;
use crate::xml::Element;

/// ft·lbs per N·m, as `FGBrushLessDCMotor.h` rounds it.
const NM_TO_FTPOUND: f64 = 1.3558;
/// W per hp.
const HP_TO_WATTS: f64 = 745.7;
/// Torque in ft·lbs of 1 W at 1 rpm.
const WATT_PER_RPM_TO_FTPOUND: f64 = 60.0 / (2.0 * PI * NM_TO_FTPOUND);

/// `FGBrushLessDCMotor`'s members beyond `FGEngine`'s.
#[derive(Debug, Clone)]
pub struct BrushLessDcMotor {
    /// Engine number, for its inputs.
    pub number: usize,
    /// Current at no load, A.
    pub zero_torque_current: f64,
    /// Ohm.
    pub coil_resistance: f64,
    /// Battery voltage, V.
    pub max_volts: f64,
    /// Speed constant, rpm/V.
    pub kv: f64,
    /// Maximum power, W.
    pub power_watts: f64,
    /// Output, hp.
    pub hp: f64,
    /// A.
    pub current: f64,
}

fn motor(m: &Models, n: usize) -> Option<&BrushLessDcMotor> {
    match &m.propulsion.engines.get(n)?.kind {
        EngineKind::BrushLessDcMotor(e) => Some(e),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

fn motor_mut(m: &mut Models, n: usize) -> Option<&mut BrushLessDcMotor> {
    match &mut m.propulsion.engines.get_mut(n)?.kind {
        EngineKind::BrushLessDcMotor(e) => Some(e),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

type Get = fn(&BrushLessDcMotor) -> f64;
type Set = fn(&mut BrushLessDcMotor, f64);

/// The constructor's ties, to `HP` and `Current`.
const MEMBERS: [(&str, Get, Set); 2] = [
    ("power-hp", |e| e.hp, |e, v| e.hp = v),
    ("current-amperes", |e| e.current, |e, v| e.current = v),
];

fn bind(b: &mut Binder<'_>, n: usize) -> Result<(), Error> {
    for (name, get, set) in MEMBERS {
        b.property(
            &format!("propulsion/engine[{n}]/{name}"),
            Kind::Double,
            move |m| motor(m, n).map_or(0.0, get),
            move |m, v| {
                if let Some(e) = motor_mut(m, n) {
                    set(e, v);
                }
            },
        )?;
    }
    Ok(())
}

/// A mandatory parameter `name` of `el`, converted to `unit`.
fn mandatory(el: &Element, name: &str, unit: Option<&str>) -> Result<f64, Error> {
    if el.find_element(name).is_none() {
        return Err(Error::Model {
            message: format!("{}: <{name}> is a mandatory parameter", el.location()),
        });
    }
    Ok(match unit {
        Some(unit) => el.find_element_value_as_number_convert_to(name, unit)?,
        None => el.find_element_value_as_number(name)?,
    })
}

impl EngineFamily for BrushLessDcMotor {
    const TYPE: EngineType = EngineType::Electric;
    type Sample = ThrusterSample;

    /// `FGBrushLessDCMotor(exec, el, n, input)`: `FGEngine::Load`, then
    /// the mandatory motor constants.
    fn load(el: &Element, cx: &mut EngineLoad<'_>) -> Result<Self, Error> {
        cx.load_base(el, EngineType::Unknown)?;
        let max_volts = mandatory(el, "maxvolts", Some("VOLTS"))?;
        let kv = mandatory(el, "velocityconstant", None)?;
        let coil_resistance = mandatory(el, "coilresistance", Some("OHMS"))?;
        let zero_torque_current = mandatory(el, "noloadcurrent", Some("AMPERES"))?;
        let max_current = max_volts / coil_resistance + zero_torque_current;
        let n = cx.number();
        bind(&mut cx.context().binder, n)?;
        Ok(Self {
            number: n,
            zero_torque_current,
            coil_resistance,
            max_volts,
            kv,
            power_watts: max_current * max_volts,
            hp: 0.0,
            current: 0.0,
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

    /// `FGBrushLessDCMotor::Calculate` between the pre- and
    /// post-functions.
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
        let rpm = thruster.rpm();
        let v = self.max_volts * input.throttle_pos.get(n).copied().unwrap_or(0.0);
        // Equation (4) of Drela's "First-Order DC Electric Motor Model".
        self.current = (v - rpm / self.kv) / self.coil_resistance;
        let i0 = self.zero_torque_current;
        let friction_current = if rpm > 0.0 {
            i0
        } else if rpm < 0.0 {
            -i0
        } else {
            constrain(-i0, self.current, i0)
        };
        let torque = (self.current - friction_current) / self.kv * WATT_PER_RPM_TO_FTPOUND;
        // `std::max(RPM, 0.0001)`.
        let turning = if rpm < 0.0001 { 0.0001 } else { rpm };
        let engine_power = ((2.0 * PI) * turning * torque) / 60.0;
        self.hp = engine_power / HP_TO_WATTS * NM_TO_FTPOUND;
        thruster.base.input.load(input);
        thruster.calculate(engine_power, &sample);
    }

    /// `FGBrushLessDCMotor::CalcFuelNeed`: none, and nothing burnt is
    /// recorded.
    fn calc_fuel_need(&mut self, _base: &mut EngineBase, _input: &Inputs) -> f64 {
        0.0
    }
}

crate::snapshot::visit_fields! {
    BrushLessDcMotor {
        number, zero_torque_current, coil_resistance, max_volts, kv, power_watts, hp, current,
    }
}
