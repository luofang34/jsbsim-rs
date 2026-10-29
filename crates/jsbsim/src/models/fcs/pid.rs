// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2006 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGPID.{h,cpp} and
// src/models/FGFCS.cpp (Load's <integrator>).
//
//! `<pid>` and `<integrator>`. An integrator is a PID whose `<c1>` is its
//! `<ki>`, integrating trapezoidally unless `<c1>` names a `type`.

use super::Kind;
use super::component::{Base, Tick, model_error, own_property, parameter_value};
use super::families::{self, Component, ComponentAt, Family};
use crate::Error;
use crate::fdm::ModelEvaluator;
use crate::fdm::{self, LoadContext};
use crate::functions::{Parameter, PropertyValue};
use crate::xml::Element;

/// The integration scheme of the `ki` term (`eIntegrateType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integration {
    /// No `<ki>`: nothing is integrated.
    None,
    /// `rect`: rectangular (Euler).
    Rect,
    /// `trap`: trapezoidal.
    Trapezoidal,
    /// `ab2` and any other type: second order Adams-Bashforth.
    AdamsBashforth2,
    /// `ab3`: third order Adams-Bashforth.
    AdamsBashforth3,
}

/// `FGPID`.
#[derive(Debug, Clone)]
pub struct Pid {
    /// `<kp>`, 0 by default.
    pub kp: Parameter,
    /// `<ki>` (an integrator's `<c1>`), 0 by default.
    pub ki: Parameter,
    /// `<kd>`, 0 by default.
    pub kd: Parameter,
    /// `<pvdot>`: the derivative used instead of differencing the input.
    pub pvdot: Option<PropertyValue>,
    /// `<trigger>`: non-zero holds the integrator, negative resets it.
    pub trigger: Option<PropertyValue>,
    /// `type="standard"`: `kp` scales every term.
    pub standard: bool,
    /// How the `ki` term integrates.
    pub integration: Integration,
    /// `I_out_total`.
    pub i_out_total: f64,
    /// `Input_prev`.
    pub input_prev: f64,
    /// `Input_prev2`.
    pub input_prev2: f64,
}

/// What one run reads.
#[derive(Debug, Clone, Copy)]
pub struct Sample {
    input: f64,
    pvdot: Option<f64>,
    trigger: f64,
    ki: f64,
    kp: f64,
    kd: f64,
}

fn integration(type_attribute: &str) -> Integration {
    match type_attribute {
        "rect" => Integration::Rect,
        "trap" => Integration::Trapezoidal,
        "ab3" => Integration::AdamsBashforth3,
        _ => Integration::AdamsBashforth2,
    }
}

/// An optional gain: its parameter, 0 when absent.
fn gain(el: Option<&Element>, ctx: &mut LoadContext<'_>) -> Result<Parameter, Error> {
    match el {
        Some(el) => parameter_value(el, ctx),
        None => Ok(Parameter::Real(0.0)),
    }
}

fn property(
    el: Option<&Element>,
    ctx: &mut LoadContext<'_>,
) -> Result<Option<PropertyValue>, Error> {
    el.map(|el| PropertyValue::new(el.data_line(0), ctx.tree(), el.location()))
        .transpose()
}

/// The `ki` element and its scheme: an integrator's `<c1>`, typed `trap`
/// unless it says otherwise.
fn ki_element(el: &Element) -> Result<Option<(&Element, Integration)>, Error> {
    if el.name() != "integrator" {
        return Ok(el
            .find_element("ki")
            .map(|ki| (ki, integration(ki.attribute("type")))));
    }
    let Some(c1) = el.find_element("c1") else {
        return Err(model_error(
            el,
            format!(
                "INTEGRATOR component {} does not provide the parameter <c1>",
                el.attribute("name")
            ),
        ));
    };
    let scheme = match c1.has_attribute("type") {
        true => integration(c1.attribute("type")),
        false => Integration::Trapezoidal,
    };
    Ok(Some((c1, scheme)))
}

/// `SetInitialOutput`, tied write-only to `<name>/initial-integrator-value`.
fn tie_initial_output(
    base: &Base,
    ctx: &mut LoadContext<'_>,
    at: ComponentAt,
) -> Result<(), Error> {
    let path = format!("{}/initial-integrator-value", own_property(&base.name));
    ctx.binder
        .setter_only(&path, fdm::Kind::Double, move |m, v| {
            if let Some(Component {
                base,
                kind: Kind::Pid(pid),
            }) = families::component_mut(&mut m.fcs, at)
            {
                pid.i_out_total = v;
                base.output = v;
            }
        })?;
    Ok(())
}

impl Family for Pid {
    type Sample = Sample;

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        at: ComponentAt,
    ) -> Result<Self, Error> {
        let ki_el = ki_element(el)?;
        base.check_input_nodes(1, 1, el, ctx)?;
        let standard = el.attribute("type") == "standard";
        let kp = gain(el.find_element("kp"), ctx)?;
        let (ki, integration) = match ki_el {
            Some((ki_el, scheme)) => (parameter_value(ki_el, ctx)?, scheme),
            None => (Parameter::Real(0.0), Integration::None),
        };
        let kd = gain(el.find_element("kd"), ctx)?;
        let pvdot = property(el.find_element("pvdot"), ctx)?;
        let trigger = property(el.find_element("trigger"), ctx)?;
        base.bind(ctx)?;
        tie_initial_output(base, ctx, at)?;
        Ok(Self {
            kp,
            ki,
            kd,
            pvdot,
            trigger,
            standard,
            integration,
            i_out_total: 0.0,
            input_prev: 0.0,
            input_prev2: 0.0,
        })
    }

    fn sample(&self, base: &Base, ev: &mut ModelEvaluator<'_>) -> Result<Sample, Error> {
        let input = base.read_input(0, ev)?;
        let pvdot = match &self.pvdot {
            Some(p) => Some(ev.property(p)?),
            None => None,
        };
        let trigger = match &self.trigger {
            Some(t) => ev.property(t)?,
            None => 0.0,
        };
        Ok(Sample {
            input,
            pvdot,
            trigger,
            ki: ev.parameter(&self.ki)?,
            kp: ev.parameter(&self.kp)?,
            kd: ev.parameter(&self.kd)?,
        })
    }

    fn step(&mut self, base: &mut Base, s: Sample, tick: &Tick) {
        let (input, dt, test) = (s.input, base.dt, s.trigger);
        base.input = input;
        let dval = s.pvdot.unwrap_or((input - self.input_prev) / dt);
        let mut i_out_delta = 0.0;
        if test.abs() < 0.000001 {
            i_out_delta = match self.integration {
                Integration::Rect => input,
                Integration::Trapezoidal => 0.5 * (input + self.input_prev),
                Integration::AdamsBashforth2 => 1.5 * input - 0.5 * self.input_prev,
                Integration::AdamsBashforth3 => {
                    (23.0 * input - 16.0 * self.input_prev + 5.0 * self.input_prev2) / 12.0
                }
                Integration::None => 0.0,
            };
        }
        if test < 0.0 {
            self.i_out_total = 0.0;
        }
        self.i_out_total += s.ki * dt * i_out_delta;
        base.output = if self.standard {
            s.kp * (input + self.i_out_total + s.kd * dval)
        } else {
            s.kp * input + self.i_out_total + s.kd * dval
        };
        self.input_prev2 = if test < 0.0 { 0.0 } else { self.input_prev };
        self.input_prev = input;
        base.clip(tick);
    }

    fn reset(&mut self, base: &mut Base) {
        base.reset_past_states();
        self.input_prev = 0.0;
        self.input_prev2 = 0.0;
        base.output = 0.0;
        self.i_out_total = 0.0;
    }
}
