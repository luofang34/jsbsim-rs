// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2007 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGActuator.{h,cpp}.
//
//! `<actuator>`: the input through lag, rate limits, deadband,
//! hysteresis, bias and the transport delay, then clipped, with the
//! `fail_zero`, `fail_hardover` and `fail_stuck` malfunctions and the
//! `saturated` flag as properties.

#[cfg(test)]
mod tests;

use super::Kind;
use super::component::{Base, Tick, own_property};
use super::families::{self, Component, ComponentAt, Family};
use crate::Error;
use crate::fdm::ModelEvaluator;
use crate::fdm::{self, LoadContext, Models};
use crate::functions::Parameter;
use crate::xml::Element;

/// `FGActuator`.
#[derive(Debug, Clone)]
pub struct Actuator {
    /// `<bias>`, a number.
    pub bias: f64,
    /// The increasing rate limit, if any.
    pub rate_limit_incr: Option<Parameter>,
    /// The decreasing rate limit, if any; a limit without `sense` is both.
    pub rate_limit_decr: Option<Parameter>,
    /// `<hysteresis_width>`, a number.
    pub hysteresis_width: f64,
    /// `<deadband_width>`, a number.
    pub deadband_width: f64,
    /// `<lag>`, a number or a property.
    pub lag: Option<Parameter>,
    /// The lag value the coefficients were computed for (`lagVal`).
    pub lag_val: f64,
    /// Lag filter coefficient `ca`.
    pub ca: f64,
    /// Lag filter coefficient `cb`.
    pub cb: f64,
    /// The previous output, held by `fail_stuck`.
    pub previous_output: f64,
    /// The previous hysteresis output.
    pub previous_hyst_output: f64,
    /// The previous rate limited output.
    pub previous_rate_lim_output: f64,
    /// The previous lag input.
    pub previous_lag_input: f64,
    /// The previous lag output.
    pub previous_lag_output: f64,
    /// `malfunction/fail_zero`.
    pub fail_zero: bool,
    /// `malfunction/fail_hardover`.
    pub fail_hardover: bool,
    /// `malfunction/fail_stuck`.
    pub fail_stuck: bool,
    /// Whether a run has happened since the last trim frame.
    pub initialized: bool,
    /// `saturated`.
    pub saturated: bool,
    /// `<clipto>` gave `<min>` but no `<max>`: JSBSim's lower limit is
    /// then a null pointer, which `fail_hardover` dereferences for a
    /// negative input.
    pub clip_min_missing: bool,
}

/// What one run reads.
#[derive(Debug, Clone, Copy)]
pub struct Sample {
    input: f64,
    lag: Option<f64>,
    rate_limit_incr: Option<f64>,
    rate_limit_decr: Option<f64>,
}

/// The `<rate_limit>` elements, the last of each sense winning; a `sense`
/// other than `incr...` or `decr...` sets neither.
fn rate_limits(
    el: &Element,
    ctx: &mut LoadContext<'_>,
) -> Result<(Option<Parameter>, Option<Parameter>), Error> {
    let (mut incr, mut decr) = (None, None);
    for limit_el in el.find_elements("rate_limit") {
        let limit =
            Parameter::number_or_property(limit_el.data_line(0), ctx.tree(), limit_el.location())?;
        if limit_el.has_attribute("sense") {
            let sense = limit_el.attribute("sense");
            if sense.starts_with("incr") {
                incr = Some(limit);
            } else if sense.starts_with("decr") {
                decr = Some(limit);
            }
        } else {
            incr = Some(limit.clone());
            decr = Some(limit);
        }
    }
    Ok((incr, decr))
}

/// The lag value `InitializeLagCoefficients` reads at load. A missing
/// property fails as in JSBSim; a tied one is not read here and gives NaN,
/// which only makes the first lagged run recompute the coefficients from
/// the value it reads, as it does whenever the value differs.
fn load_lag_value(lag: &Parameter, ctx: &mut LoadContext<'_>) -> Result<f64, Error> {
    match lag {
        Parameter::Real(v) => Ok(*v),
        Parameter::Property(p) => {
            let node = p.node(ctx.tree())?;
            if ctx.tree().is_tied(node) {
                return Ok(f64::NAN);
            }
            Ok(ctx.tree().get_double(node) * p.sign())
        }
        _ => Ok(f64::NAN),
    }
}

/// `<clipto>` with `<min>` and without `<max>`.
fn clip_min_missing(el: &Element) -> bool {
    el.find_element("clipto")
        .is_some_and(|c| c.find_element("min").is_some() && c.find_element("max").is_none())
}

fn actuator(m: &Models, at: ComponentAt) -> Option<&Actuator> {
    match &m
        .fcs
        .channels
        .get(at.channel)?
        .components
        .get(at.index)?
        .kind
    {
        Kind::Actuator(a) => Some(a),
        _ => None,
    }
}

fn actuator_mut(m: &mut Models, at: ComponentAt) -> Option<&mut Actuator> {
    match families::component_mut(&mut m.fcs, at) {
        Some(Component {
            kind: Kind::Actuator(a),
            ..
        }) => Some(a),
        _ => None,
    }
}

/// `bind`: the malfunctions and the saturation flag under the
/// component's own property.
fn bind(base: &Base, ctx: &mut LoadContext<'_>, at: ComponentAt) -> Result<(), Error> {
    type Get = fn(&Actuator) -> bool;
    type Set = fn(&mut Actuator, bool);
    let own = own_property(&base.name);
    let flags: [(&str, Get, Set); 3] = [
        ("fail_zero", |a| a.fail_zero, |a, v| a.fail_zero = v),
        (
            "fail_hardover",
            |a| a.fail_hardover,
            |a, v| a.fail_hardover = v,
        ),
        ("fail_stuck", |a| a.fail_stuck, |a, v| a.fail_stuck = v),
    ];
    for (name, get, set) in flags {
        ctx.binder.property(
            &format!("{own}/malfunction/{name}"),
            fdm::Kind::Bool,
            move |m| actuator(m, at).map_or(0.0, |a| f64::from(u8::from(get(a)))),
            move |m, v| {
                if let Some(a) = actuator_mut(m, at) {
                    set(a, v != 0.0);
                }
            },
        )?;
    }
    ctx.binder
        .typed_getter(&format!("{own}/saturated"), fdm::Kind::Bool, move |m| {
            actuator(m, at).map_or(0.0, |a| f64::from(u8::from(a.saturated)))
        })?;
    Ok(())
}

impl Actuator {
    /// `Lag`, with the lag value read this run when initialized.
    fn lag(&mut self, base: &mut Base, value: Option<f64>) {
        let input = base.output;
        if let (true, Some(value)) = (self.initialized, value) {
            if self.lag_val != value {
                self.initialize_lag_coefficients(value, base.dt);
            }
            base.output =
                self.ca * (input + self.previous_lag_input) + self.previous_lag_output * self.cb;
        }
        self.previous_lag_input = input;
        self.previous_lag_output = base.output;
    }

    /// `InitializeLagCoefficients`.
    fn initialize_lag_coefficients(&mut self, value: f64, dt: f64) {
        self.lag_val = value;
        let denom = 2.00 + dt * value;
        self.ca = dt * value / denom;
        self.cb = (2.00 - dt * value) / denom;
    }

    /// `RateLimit`.
    fn rate_limit(&mut self, base: &mut Base, s: &Sample) {
        let input = base.output;
        let dt = base.dt;
        if self.initialized {
            let delta = input - self.previous_rate_lim_output;
            if let Some(rate_limit) = s.rate_limit_incr
                && delta > dt * rate_limit
            {
                base.output = self.previous_rate_lim_output + rate_limit * dt;
            }
            if let Some(decr) = s.rate_limit_decr {
                let rate_limit = -decr;
                if delta < dt * rate_limit {
                    base.output = self.previous_rate_lim_output + rate_limit * dt;
                }
            }
        }
        self.previous_rate_lim_output = base.output;
    }

    /// `Deadband`.
    fn deadband(&self, base: &mut Base) {
        let input = base.output;
        let width = self.deadband_width;
        base.output = if input < -width / 2.0 {
            input + width / 2.0
        } else if input > width / 2.0 {
            input - width / 2.0
        } else {
            0.0
        };
    }

    /// `Hysteresis`.
    fn hysteresis(&mut self, base: &mut Base) {
        let input = base.output;
        if self.initialized {
            let half = 0.5 * self.hysteresis_width;
            if input > self.previous_hyst_output {
                base.output = cpp_max(self.previous_hyst_output, input - half);
            } else if input < self.previous_hyst_output {
                base.output = cpp_min(self.previous_hyst_output, input + half);
            }
        }
        self.previous_hyst_output = base.output;
    }

    /// The processing of a healthy (not stuck) actuator, in JSBSim's
    /// order.
    fn process(&mut self, base: &mut Base, s: &Sample, tick: &Tick) {
        if self.lag.is_some() {
            self.lag(base, s.lag);
        }
        if self.rate_limit_incr.is_some() || self.rate_limit_decr.is_some() {
            self.rate_limit(base, s);
        }
        if self.deadband_width != 0.0 {
            self.deadband(base);
        }
        if self.hysteresis_width != 0.0 {
            self.hysteresis(base);
        }
        if self.bias != 0.0 {
            base.output += self.bias;
        }
        if !base.delay.is_empty() {
            base.delay(tick);
        }
    }

    /// The saturation flag after clipping; only a clipping actuator
    /// updates it, and a zero limit never saturates.
    fn update_saturation(&mut self, base: &Base, tick: &Tick) {
        let Some((clipmin, clipmax)) = tick.clip else {
            return;
        };
        self.saturated = (base.output >= clipmax && clipmax != 0.0)
            || (base.output <= clipmin && clipmin != 0.0);
    }
}

/// `std::max(a, b)`: `b` only when `a < b`.
fn cpp_max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// `std::min(a, b)`: `b` only when `b < a`.
fn cpp_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

impl Family for Actuator {
    type Sample = Sample;

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        at: ComponentAt,
    ) -> Result<Self, Error> {
        base.check_input_nodes(1, 1, el, ctx)?;
        let number = |name: &str| -> Result<f64, Error> {
            match el.find_element(name) {
                Some(_) => Ok(el.find_element_value_as_number(name)?),
                None => Ok(0.0),
            }
        };
        let deadband_width = number("deadband_width")?;
        let hysteresis_width = number("hysteresis_width")?;
        let (rate_limit_incr, rate_limit_decr) = rate_limits(el, ctx)?;
        let bias = number("bias")?;
        let mut actuator = Self {
            bias,
            rate_limit_incr,
            rate_limit_decr,
            hysteresis_width,
            deadband_width,
            lag: None,
            lag_val: 0.0,
            ca: 0.0,
            cb: 0.0,
            previous_output: 0.0,
            previous_hyst_output: 0.0,
            previous_rate_lim_output: 0.0,
            previous_lag_input: 0.0,
            previous_lag_output: 0.0,
            fail_zero: false,
            fail_hardover: false,
            fail_stuck: false,
            initialized: false,
            saturated: false,
            clip_min_missing: clip_min_missing(el),
        };
        if let Some(lag_el) = el.find_element("lag") {
            let lag =
                Parameter::number_or_property(lag_el.data_line(0), ctx.tree(), lag_el.location())?;
            let value = load_lag_value(&lag, ctx)?;
            actuator.initialize_lag_coefficients(value, base.dt);
            actuator.lag = Some(lag);
        }
        base.bind(ctx)?;
        bind(base, ctx, at)?;
        Ok(actuator)
    }

    fn sample(&self, base: &Base, ev: &mut ModelEvaluator<'_>) -> Result<Sample, Error> {
        let input = base.read_input(0, ev)?;
        let failed_input = if self.fail_zero { 0.0 } else { input };
        if self.fail_hardover && failed_input < 0.0 && self.clip_min_missing {
            return Err(model_error_at(base));
        }
        // The lag and the rate limits are read only by an initialized,
        // healthy actuator. JSBSim also skips them on a trim frame, which
        // the sample cannot know; reading them then differs only in when a
        // missing property is reported.
        let healthy = self.initialized && !self.fail_stuck;
        let read =
            |p: &Option<Parameter>, ev: &mut ModelEvaluator<'_>| -> Result<Option<f64>, Error> {
                match (healthy, p) {
                    (true, Some(p)) => Ok(Some(ev.parameter(p)?)),
                    _ => Ok(None),
                }
            };
        let lag = read(&self.lag, ev)?;
        let rate_limit_incr = read(&self.rate_limit_incr, ev)?;
        let rate_limit_decr = read(&self.rate_limit_decr, ev)?;
        Ok(Sample {
            input,
            lag,
            rate_limit_incr,
            rate_limit_decr,
        })
    }

    fn step(&mut self, base: &mut Base, s: Sample, tick: &Tick) {
        base.input = s.input;
        if tick.trim_status {
            self.initialized = false;
        }
        if self.fail_zero {
            base.input = 0.0;
        }
        if self.fail_hardover {
            // Without <clipto> both limits are JSBSim's default 0.
            let (clipmin, clipmax) = tick.clip.unwrap_or((0.0, 0.0));
            base.input = if base.input < 0.0 { clipmin } else { clipmax };
        }
        base.output = base.input;
        if self.fail_stuck {
            base.output = self.previous_output;
        } else {
            self.process(base, &s, tick);
        }
        self.previous_output = base.output;
        self.initialized = true;
        base.clip(tick);
        self.update_saturation(base, tick);
    }

    fn reset(&mut self, base: &mut Base) {
        base.reset_past_states();
        self.previous_output = 0.0;
        self.previous_hyst_output = 0.0;
        self.previous_rate_lim_output = 0.0;
        self.previous_lag_input = 0.0;
        self.previous_lag_output = 0.0;
        base.output = 0.0;
    }
}

/// The error for a hardover failure JSBSim would crash on.
fn model_error_at(base: &Base) -> Error {
    Error::Model {
        message: format!(
            "actuator {}: fail_hardover with a negative input reads the <clipto> <min> JSBSim discards when <max> is missing",
            base.name
        ),
    }
}

crate::snapshot::visit_fields! {
    Actuator {
        bias, hysteresis_width, deadband_width, lag_val, ca, cb, previous_output,
        previous_hyst_output, previous_rate_lim_output, previous_lag_input, previous_lag_output,
        fail_zero, fail_hardover, fail_stuck, initialized, saturated, clip_min_missing,
    }
    parts { rate_limit_incr, rate_limit_decr, lag }
}
