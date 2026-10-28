// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2019 Adriano Bassignana.
// Derived from JSBSim src/models/flight_control/FGLinearActuator.{h,cpp}.
//
//! `<linear_actuator>`: turns an input that wraps around at `<module>`
//! (an angle in 0..360, say) into a continuous output by counting the
//! wraps, with a hysteresis, an allowed direction (`<versus>`), bias,
//! gain, lag, and `<set>`/`<reset>` controls.

#[cfg(test)]
mod tests;

use super::component::{Base, Tick};
use super::families::{ComponentAt, Family};
use crate::Error;
use crate::fdm::LoadContext;
use crate::functions::{Evaluator, Parameter};
use crate::properties::PropertyTree;
use crate::xml::Element;

/// `FGLinearActuator`.
#[derive(Debug, Clone)]
pub struct LinearActuator {
    /// `<set>`: the input is followed while set.
    pub set_param: Option<Parameter>,
    /// The last `set` read (or the constant one), true by default.
    pub set: bool,
    /// `<reset>`: zeroes the state while set.
    pub reset_param: Option<Parameter>,
    /// The last `reset` read (or the constant one).
    pub reset: bool,
    /// The direction of the last small move: 1, -1 or 0.
    pub direction: i32,
    /// The wraps counted (`countSpin`).
    pub count_spin: i32,
    /// The direction allowed (`versus`): 1, -1, or 0 for both.
    pub versus: i32,
    /// `<versus>`.
    pub versus_param: Option<Parameter>,
    /// The bias (the constant `<bias>`, or the last read).
    pub bias: f64,
    /// `<bias>`.
    pub bias_param: Option<Parameter>,
    /// The input moves ignored against `versus` (`inputLast`).
    pub input_last: f64,
    /// The input last followed (`inputMem`).
    pub input_mem: f64,
    /// `<module>`: the input's period, 1 by default.
    pub module: f64,
    /// `<hysteresis>`, 0.1 by default.
    pub hysteresis: f64,
    /// `<rate>`: the fraction of the module beyond which a move is a
    /// wrap, 0.3 by default.
    pub rate: f64,
    /// `<gain>`, 1 by default.
    pub gain: f64,
    /// `<lag>`, 0 (none) by default.
    pub lag: f64,
    /// The previous lag input.
    pub previous_lag_input: f64,
    /// The previous lag output.
    pub previous_lag_output: f64,
    /// Lag filter coefficient `ca`.
    pub ca: f64,
    /// Lag filter coefficient `cb`.
    pub cb: f64,
}

/// What one run reads: each only when JSBSim's `Run` reads it.
#[derive(Debug, Clone, Copy)]
pub struct Sample {
    set: Option<f64>,
    reset: Option<f64>,
    input: Option<f64>,
    versus: Option<f64>,
    bias: Option<f64>,
}

/// `FGParameter::IsConstant`.
fn is_constant(parameter: &Parameter, tree: &PropertyTree) -> bool {
    match parameter {
        Parameter::Real(_) => true,
        Parameter::Property(p) => p.is_constant(tree),
        _ => false,
    }
}

/// `<name>` as `FGParameterValue(FindElementValue(name), pm, el)`, with
/// its value when it is constant at load.
fn parameter(
    el: &Element,
    name: &str,
    ctx: &mut LoadContext<'_>,
) -> Result<Option<(Parameter, Option<f64>)>, Error> {
    if el.find_element(name).is_none() {
        return Ok(None);
    }
    let parameter =
        Parameter::number_or_property(el.find_element_value(name), ctx.tree(), el.location())?;
    let value = match &parameter {
        Parameter::Real(v) => Some(*v),
        Parameter::Property(p) if p.is_constant(ctx.tree()) => {
            let node = p.node(ctx.tree())?;
            Some(ctx.tree().get_double(node) * p.sign())
        }
        _ => None,
    };
    Ok(Some((parameter, value)))
}

/// An optional number, forced to `fallback` (with JSBSim's warning) when
/// `invalid` says so.
fn number(
    el: &Element,
    base: &Base,
    name: &str,
    default: f64,
    invalid: fn(f64) -> bool,
    fallback: f64,
) -> Result<f64, Error> {
    if el.find_element(name).is_none() {
        return Ok(default);
    }
    let value = el.find_element_value_as_number(name)?;
    if invalid(value) {
        let input = base.inputs.first().map_or("", |i| i.name());
        tracing::warn!(
            "FGLinearActuator::Run {input} <{name}> parameter is forced from {value:.6} value to {fallback:.1} value"
        );
        return Ok(fallback);
    }
    Ok(value)
}

/// The value of a parameter JSBSim reads in `Run`: when it is not
/// constant.
fn read(p: &Option<Parameter>, ev: &mut Evaluator<'_>) -> Result<Option<f64>, Error> {
    match p {
        Some(p) if !is_constant(p, ev.tree()) => Ok(Some(ev.parameter(p)?)),
        _ => Ok(None),
    }
}

impl LinearActuator {
    /// The input followed while set and not reset (the `set` branch of
    /// `Run`).
    fn follow(&mut self, base: &mut Base, input: f64, versus: Option<f64>) {
        base.input = input - self.input_last;
        let input_delta = base.input - self.input_mem;
        if input_delta.abs() < self.hysteresis {
            return;
        }
        if let Some(v) = versus {
            // JSBSim assigns the value to its int `versus` before comparing
            // it with +-0.5, so it truncates first: 0.7 allows both ways.
            let truncated = f64::from(v as i32);
            self.versus = if truncated >= 0.5 {
                1
            } else if truncated <= -0.5 {
                -1
            } else {
                0
            };
        }
        let wrap = self.module * self.rate;
        if input_delta.abs() <= wrap {
            if input_delta > 0.0 {
                self.direction = 1;
            } else if input_delta < 0.0 {
                self.direction = -1;
            }
        }
        if self.versus == 0 || self.versus == self.direction {
            self.input_mem = base.input;
            if input_delta.abs() >= wrap {
                self.count_spin = if input_delta < 0.0 {
                    self.count_spin.wrapping_add(1)
                } else {
                    self.count_spin.wrapping_sub(1)
                };
            }
        } else if self.versus != 0 && self.direction != 0 && self.versus != self.direction {
            self.input_last += input_delta;
        }
    }
}

impl Family for LinearActuator {
    type Sample = Sample;

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        _at: ComponentAt,
    ) -> Result<Self, Error> {
        base.check_input_nodes(1, 1, el, ctx)?;
        let set = parameter(el, "set", ctx)?;
        let reset = parameter(el, "reset", ctx)?;
        let versus = parameter(el, "versus", ctx)?;
        let bias = parameter(el, "bias", ctx)?;
        let module = number(el, base, "module", 1.0, |v| v < 0.0, 1.0)?;
        let hysteresis = number(el, base, "hysteresis", 0.1, |v| v < 0.0, 0.0)?;
        let lag = number(el, base, "lag", 0.0, |v| v < 0.0, 0.0)?;
        let rate = number(el, base, "rate", 0.3, |v| v <= 0.0 || v > 1.0, 0.5)?;
        let gain = number(el, base, "gain", 1.0, |_| false, 1.0)?;
        let (mut ca, mut cb) = (0.0, 0.0);
        if lag > 0.0 {
            let dt = base.dt;
            let denom = 2.00 + dt * lag;
            ca = dt * lag / denom;
            cb = (2.00 - dt * lag) / denom;
        }
        let value = |p: &Option<(Parameter, Option<f64>)>| p.as_ref().and_then(|(_, v)| *v);
        let actuator = Self {
            set: value(&set).is_none_or(|v| v >= 0.5),
            reset: value(&reset).is_some_and(|v| v >= 0.5),
            // `versus` is an int in JSBSim: a constant truncates.
            versus: value(&versus).map_or(0, |v| v as i32),
            bias: value(&bias).unwrap_or(0.0),
            set_param: set.map(|(p, _)| p),
            reset_param: reset.map(|(p, _)| p),
            versus_param: versus.map(|(p, _)| p),
            bias_param: bias.map(|(p, _)| p),
            direction: 0,
            count_spin: 0,
            input_last: 0.0,
            input_mem: 0.0,
            module,
            hysteresis,
            rate,
            gain,
            lag,
            previous_lag_input: 0.0,
            previous_lag_output: 0.0,
            ca,
            cb,
        };
        base.bind(ctx)?;
        Ok(actuator)
    }

    fn sample(&self, base: &Base, ev: &mut Evaluator<'_>) -> Result<Sample, Error> {
        let set = read(&self.set_param, ev)?;
        let reset = read(&self.reset_param, ev)?;
        let mut s = Sample {
            set,
            reset,
            input: None,
            versus: None,
            bias: None,
        };
        if reset.map_or(self.reset, |v| v >= 0.5) {
            return Ok(s);
        }
        if set.map_or(self.set, |v| v >= 0.5) {
            let input = base.read_input(0, ev)?;
            s.input = Some(input);
            let input_delta = (input - self.input_last) - self.input_mem;
            if input_delta.abs() >= self.hysteresis {
                s.versus = read(&self.versus_param, ev)?;
            }
        }
        s.bias = read(&self.bias_param, ev)?;
        Ok(s)
    }

    fn step(&mut self, base: &mut Base, s: Sample, _tick: &Tick) {
        if let Some(v) = s.set {
            self.set = v >= 0.5;
        }
        if let Some(v) = s.reset {
            self.reset = v >= 0.5;
        }
        if self.reset {
            self.input_mem = 0.0;
            self.count_spin = 0;
            self.direction = 0;
            base.output = 0.0;
            self.input_last = 0.0;
        } else {
            if let (true, Some(input)) = (self.set, s.input) {
                self.follow(base, input, s.versus);
            }
            if let Some(bias) = s.bias {
                self.bias = bias;
            }
            base.output =
                self.gain * (self.bias + self.input_mem + self.module * f64::from(self.count_spin));
        }
        if self.lag > 0.0 {
            let input = base.output;
            base.output =
                self.ca * (input + self.previous_lag_input) + self.previous_lag_output * self.cb;
            self.previous_lag_input = input;
            self.previous_lag_output = base.output;
        }
    }
}
