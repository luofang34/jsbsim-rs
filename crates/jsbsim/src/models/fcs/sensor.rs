// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2005 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGSensor.{h,cpp}.
//
//! `<sensor>`: an input degraded by lag, drift, gain, bias, a transport
//! delay, failures and quantization. [`Signal`] is the degradation every
//! sensor family shares (`FGSensor`'s members and `ProcessSensorSignal`).
//!
//! A sensor whose `<noise>` is non-zero draws one random number on every
//! run, from the executive's generator until its `randomseed` property
//! gives it a generator of its own.

mod bind;
pub mod orientation;
#[cfg(test)]
mod tests;

use super::component::{Base, Tick, lenient, model_error};
use super::families::{ComponentAt, Family};
use crate::Error;
use crate::fdm::LoadContext;
use crate::fdm::ModelEvaluator;
use crate::random::RandomNumberGenerator;
use crate::xml::Element;

/// `<noise>`: `noise_variance`, `NoiseType` and `DistributionType`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Noise {
    /// The `<noise>` value; 0 draws nothing.
    pub variance: f64,
    /// `eAbsolute`: the deviate scaled by the variance is added; otherwise
    /// (`ePercent`) the output is scaled by one plus it.
    pub absolute: bool,
    /// `eGaussian`: a normal deviate; otherwise (`eUniform`) a uniform
    /// one over `[-1, 1)`.
    pub gaussian: bool,
}

/// `FGSensor`'s members.
#[derive(Debug, Clone, Default)]
pub struct Signal {
    /// `<quantization>` `<min>`.
    pub min: f64,
    /// `<quantization>` `<max>`.
    pub max: f64,
    /// `<bias>`.
    pub bias: f64,
    /// `<gain>`; 0 applies none.
    pub gain: f64,
    /// `<drift_rate>`, per second.
    pub drift_rate: f64,
    /// Drift accumulated so far; a reset keeps it.
    pub drift: f64,
    /// `<lag>`.
    pub lag: f64,
    /// Quantization step.
    pub granularity: f64,
    /// Lag filter coefficient `a`.
    pub ca: f64,
    /// Lag filter coefficient `b`.
    pub cb: f64,
    /// Lag filter output of the previous run.
    pub previous_output: f64,
    /// Lag filter input of the previous run.
    pub previous_input: f64,
    /// `<quantization>` `<bits>`; 0 does not quantize.
    pub bits: i32,
    /// Quantization steps above `min` of the last quantized output.
    pub quantized: i32,
    /// `1 << bits`.
    pub divisions: i32,
    /// `malfunction/fail_low`: the output is minus infinity.
    pub fail_low: bool,
    /// `malfunction/fail_high`: the output is plus infinity.
    pub fail_high: bool,
    /// `malfunction/fail_stuck`: the output keeps its last value.
    pub fail_stuck: bool,
    /// `<noise>`.
    pub noise: Noise,
    /// The sensor's own seed once `randomseed` is written.
    pub random_seed: Option<i32>,
    /// The generator `SetNoiseRandomSeed` makes from that seed.
    pub generator: Option<RandomNumberGenerator>,
}

/// `(int)` of a `<bits>` value, refusing the values for which C++'s
/// conversion or `1 << bits` is undefined.
fn bits(el: &Element, value: f64) -> Result<i32, Error> {
    if !(value > -1.0 && value < 31.0) {
        return Err(model_error(
            el,
            format!("Invalid quantization of {value} bits"),
        ));
    }
    Ok(value as i32)
}

impl Signal {
    /// `FGSensor(fcs, element)` after the base constructor, `bind`
    /// included.
    pub fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        at: ComponentAt,
    ) -> Result<Self, Error> {
        let mut s = Self::default();
        let mut quant_property = "";
        if let Some(q) = el.find_element("quantization") {
            if let Some(bits_el) = q.find_element("bits") {
                s.bits = bits(bits_el, q.find_element_value_as_number("bits")?)?;
            }
            s.divisions = 1 << s.bits;
            if q.find_element("min").is_some() {
                s.min = q.find_element_value_as_number("min")?;
            }
            if q.find_element("max").is_some() {
                s.max = q.find_element_value_as_number("max")?;
            }
            quant_property = q.attribute("name");
            s.granularity = (s.max - s.min) / f64::from(s.divisions);
        }
        let number = |name: &str| match el.find_element(name) {
            Some(_) => el.find_element_value_as_number(name).map_err(Error::from),
            None => Ok(0.0),
        };
        s.bias = number("bias")?;
        s.gain = number("gain")?;
        s.drift_rate = number("drift_rate")?;
        if el.find_element("lag").is_some() {
            s.lag = el.find_element_value_as_number("lag")?;
            let denom = 2.00 + base.dt * s.lag;
            s.ca = base.dt * s.lag / denom;
            s.cb = (2.00 - base.dt * s.lag) / denom;
        }
        if let Some(noise_el) = el.find_element("noise") {
            s.noise = noise(el, noise_el, &base.name, ctx)?;
        }
        bind::bind(el, base, quant_property, ctx, at)?;
        Ok(s)
    }

    /// `ProcessSensorSignal`, from `base.input`.
    pub fn process(&mut self, base: &mut Base, tick: &Tick) {
        if self.fail_stuck {
            return;
        }
        base.output = base.input;
        if self.lag != 0.0 {
            base.output =
                self.ca * (base.output + self.previous_input) + self.previous_output * self.cb;
            self.previous_output = base.output;
            self.previous_input = base.input;
        }
        if self.noise.variance != 0.0 {
            self.apply_noise(base, tick.rng);
        }
        if self.drift_rate != 0.0 {
            self.drift += self.drift_rate * base.dt;
            base.output += self.drift;
        }
        if self.gain != 0.0 {
            base.output *= self.gain;
        }
        if self.bias != 0.0 {
            base.output += self.bias;
        }
        if !base.delay.is_empty() {
            base.delay(tick);
        }
        if self.fail_low {
            base.output = -f64::INFINITY;
        }
        if self.fail_high {
            base.output = f64::INFINITY;
        }
        if self.bits != 0 {
            self.quantize(base);
        }
        base.clip(tick);
    }

    /// `Noise`, from the sensor's own generator if it has one.
    fn apply_noise(&self, base: &mut Base, exec_rng: &RandomNumberGenerator) {
        let rng = self.generator.as_ref().unwrap_or(exec_rng);
        let random_value = if self.noise.gaussian {
            rng.normal()
        } else {
            rng.uniform()
        };
        if self.noise.absolute {
            base.output += self.noise.variance * random_value;
        } else {
            base.output *= 1.0 + self.noise.variance * random_value;
        }
    }

    /// `SetNoiseRandomSeed`: a new generator of the sensor's own.
    pub fn set_random_seed(&mut self, seed: i32) {
        self.random_seed = Some(seed);
        self.generator = Some(RandomNumberGenerator::new(seed as u32));
    }

    /// `Quantize`. `(int)` of an out-of-range quotient is undefined in
    /// C++; Rust's saturating conversion is what arm64 hardware does.
    fn quantize(&mut self, base: &mut Base) {
        if base.output < self.min {
            base.output = self.min;
        }
        if base.output > self.max {
            base.output = self.max;
        }
        let portion = base.output - self.min;
        self.quantized = (portion / self.granularity) as i32;
        base.output = f64::from(self.quantized) * self.granularity + self.min;
    }

    /// `FGSensor::ResetPastStates`.
    pub fn reset(&mut self, base: &mut Base) {
        base.reset_past_states();
        self.previous_output = 0.0;
        self.previous_input = 0.0;
        base.output = 0.0;
    }
}

/// `<noise>`: an unknown variation is `PERCENT` and an unknown
/// distribution `UNIFORM`, each logged.
fn noise(
    el: &Element,
    noise_el: &Element,
    name: &str,
    ctx: &LoadContext<'_>,
) -> Result<Noise, Error> {
    let variance = el.find_element_value_as_number("noise")?;
    let variation = noise_el.attribute("variation");
    let distribution = noise_el.attribute("distribution");
    if !matches!(variation, "PERCENT" | "ABSOLUTE") {
        lenient(
            noise_el,
            ctx,
            &format!("Unknown noise type in sensor: {name}\n  defaulting to PERCENT."),
        )?;
    }
    if !matches!(distribution, "UNIFORM" | "GAUSSIAN") {
        lenient(
            noise_el,
            ctx,
            &format!(
                "Unknown random distribution type in sensor: {name}\n  defaulting to UNIFORM."
            ),
        )?;
    }
    Ok(Noise {
        variance,
        absolute: variation == "ABSOLUTE",
        gaussian: distribution == "GAUSSIAN",
    })
}

/// The error of a sensor family's [`Family::sample`], which needs the
/// models that only [`Family::sample_models`] is given.
pub(super) fn needs_models(base: &Base) -> Error {
    Error::Model {
        message: format!("Sensor {} reads the models it was not given", base.name),
    }
}

/// `FGSensor`.
#[derive(Debug, Clone)]
pub struct Sensor {
    /// The degradation.
    pub signal: Signal,
}

impl Family for Sensor {
    /// `InputNodes[0]`.
    type Sample = f64;

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        at: ComponentAt,
    ) -> Result<Self, Error> {
        Ok(Self {
            signal: Signal::load(el, base, ctx, at)?,
        })
    }

    fn sample(&self, base: &Base, ev: &mut ModelEvaluator<'_>) -> Result<f64, Error> {
        match base.inputs.first() {
            Some(input) => ev.property(input),
            // JSBSim dereferences the missing input here.
            None => Err(Error::Model {
                message: format!("Sensor {} has no <input>", base.name),
            }),
        }
    }

    fn step(&mut self, base: &mut Base, input: f64, tick: &Tick) {
        base.input = input;
        self.signal.process(base, tick);
    }

    fn reset(&mut self, base: &mut Base) {
        self.signal.reset(base);
    }
}

crate::snapshot::visit_fields! { Noise { variance, absolute, gaussian } }
crate::snapshot::visit_fields! {
    Signal {
        min, max, bias, gain, drift_rate, drift, lag, granularity, ca, cb, previous_output,
        previous_input, bits, quantized, divisions, fail_low, fail_high, fail_stuck, noise,
        random_seed, generator,
    }
}
crate::snapshot::visit_fields! { Sensor { signal } }
