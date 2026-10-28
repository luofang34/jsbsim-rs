// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGGain.{h,cpp}.
//
//! `<pure_gain>`, `<scheduled_gain>` and `<aerosurface_scale>`.

use super::component::{Base, Tick, lenient, model_error, parameter_value};
use super::families::{ComponentAt, Family};
use crate::Error;
use crate::fdm::LoadContext;
use crate::functions::{Evaluator, Parameter, TableId};
use crate::xml::Element;

/// `<aerosurface_scale>`'s mapping of the domain onto the range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scale {
    /// `<domain><min>`, -1 by default.
    pub in_min: f64,
    /// `<domain><max>`, 1 by default.
    pub in_max: f64,
    /// `<range><min>`.
    pub out_min: f64,
    /// `<range><max>`.
    pub out_max: f64,
    /// Each sign of the input scales by its own end of the domain and
    /// range, so 0 maps to 0.
    pub zero_centered: bool,
}

/// Which gain.
#[derive(Debug, Clone, PartialEq)]
pub enum GainType {
    /// `<pure_gain>`.
    Pure,
    /// `<scheduled_gain>` and its table.
    Scheduled(TableId),
    /// `<aerosurface_scale>`.
    AerosurfaceScale(Scale),
}

/// `FGGain`.
#[derive(Debug, Clone)]
pub struct Gain {
    /// `<gain>`, 1 by default.
    pub gain: Parameter,
    /// Which gain.
    pub kind: GainType,
}

/// What one run reads.
#[derive(Debug, Clone, Copy)]
pub struct Sample {
    input: f64,
    gain: f64,
    scheduled: f64,
}

/// `<domain>` and `<range>` of an `<aerosurface_scale>`.
fn load_scale(el: &Element) -> Result<Scale, Error> {
    let mut scale = Scale {
        in_min: -1.0,
        in_max: 1.0,
        out_min: 0.0,
        out_max: 0.0,
        zero_centered: true,
    };
    let both = |e: &Element| e.find_element("max").is_some() && e.find_element("min").is_some();
    if let Some(domain) = el.find_element("domain")
        && both(domain)
    {
        scale.in_max = domain.find_element_value_as_number("max")?;
        scale.in_min = domain.find_element_value_as_number("min")?;
    }
    let Some(range) = el.find_element("range") else {
        return Err(model_error(
            el,
            "No range supplied for aerosurface scale component",
        ));
    };
    if !both(range) {
        return Err(model_error(
            range,
            "Maximum and minimum output values must be supplied for the aerosurface scale component",
        ));
    }
    scale.out_max = range.find_element_value_as_number("max")?;
    scale.out_min = range.find_element_value_as_number("min")?;
    if el.find_element("zero_centered").is_some() {
        let value = el.find_element_value("zero_centered");
        if value == "0" || value == "false" {
            scale.zero_centered = false;
        }
    }
    Ok(scale)
}

impl Family for Gain {
    type Sample = Sample;

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        _at: ComponentAt,
    ) -> Result<Self, Error> {
        base.check_input_nodes(1, 1, el, ctx)?;
        let gain_el = el.find_element("gain");
        if el.name() == "pure_gain" && gain_el.is_none() {
            lenient(el, ctx, "      No GAIN specified (default: 1.0)")?;
        }
        let gain = match gain_el {
            Some(gain_el) => parameter_value(gain_el, ctx)?,
            None => Parameter::Real(1.0),
        };
        let kind = match el.name() {
            "aerosurface_scale" => GainType::AerosurfaceScale(load_scale(el)?),
            "scheduled_gain" => {
                let Some(table) = el.find_element("table") else {
                    return Err(model_error(
                        el,
                        "A table must be provided for the scheduled gain component",
                    ));
                };
                GainType::Scheduled(ctx.functions.load_table(ctx.binder.tree, table, "")?)
            }
            _ => GainType::Pure,
        };
        base.bind(ctx)?;
        Ok(Self { gain, kind })
    }

    fn sample(&self, base: &Base, ev: &mut Evaluator<'_>) -> Result<Sample, Error> {
        let input = base.read_input(0, ev)?;
        let scheduled = match self.kind {
            GainType::Scheduled(table) => ev.table(table)?,
            _ => 0.0,
        };
        let gain = ev.parameter(&self.gain)?;
        Ok(Sample {
            input,
            gain,
            scheduled,
        })
    }

    fn step(&mut self, base: &mut Base, s: Sample, tick: &Tick) {
        let input = s.input;
        base.input = input;
        base.output = match &self.kind {
            GainType::Pure => s.gain * input,
            GainType::Scheduled(_) => s.gain * s.scheduled * input,
            GainType::AerosurfaceScale(sc) => {
                let output = if !sc.zero_centered {
                    sc.out_min
                        + ((input - sc.in_min) / (sc.in_max - sc.in_min))
                            * (sc.out_max - sc.out_min)
                } else if input == 0.0 {
                    0.0
                } else if input > 0.0 {
                    (input / sc.in_max) * sc.out_max
                } else {
                    (input / sc.in_min) * sc.out_min
                };
                output * s.gain
            }
        };
        base.clip(tick);
    }
}
