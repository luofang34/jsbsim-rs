// SPDX-License-Identifier: LGPL-2.1-or-later
// FGOutput: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// FGOutputType: Copyright (C) 2011 Bertrand Coconnier.
// Derived from JSBSim src/models/FGOutput.cpp (Load) and
// src/input_output/FGOutputType.cpp (SetIdx, Load, SetRateHz, GetRateHz).
//
//! `FGOutput` and its output types, without their files and sockets.
//!
//! Each `<output>` directive JSBSim loads becomes an output type that ties
//! `simulation/output[n]/log_rate_hz` and `simulation/output[n]/enabled`.
//! The port keeps those properties and their state exactly, so scripts can
//! write and read them, but writes nothing: output types only print, so
//! nothing in the simulation depends on them.

use super::base::ModelBase;
use crate::Error;
use crate::fdm::{Binder, Kind};
use crate::xml::Element;

#[cfg(test)]
mod tests;

/// `FGOutput`.
#[derive(Debug, Clone, Default)]
pub struct Output {
    /// Scheduling of the output model itself.
    pub base: ModelBase,
    /// The output types, by index `n` of `simulation/output[n]`.
    pub types: Vec<OutputType>,
}

/// The state of an `FGOutputType` its properties expose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputType {
    /// `FGModel::rate`: frames between prints.
    pub rate: u32,
    /// `FGModel::enabled`, tied to `simulation/output[n]/enabled`.
    pub enabled: bool,
}

impl Default for OutputType {
    fn default() -> Self {
        let base = ModelBase::default();
        Self {
            rate: base.rate,
            enabled: base.enabled,
        }
    }
}

impl OutputType {
    /// `SetRateHz(rtHz)` at time step `dt`: clamps to [0, 1000] Hz; a
    /// positive rate enables the output, zero disables it.
    pub fn set_rate_hz(&mut self, rt_hz: f64, dt: f64) {
        // NaN stays NaN, as in C++, and so disables.
        let rt_hz = rt_hz.clamp(0.0, 1000.0);
        if rt_hz > 0.0 {
            // C++ converts the double to `unsigned int` as `as` does on
            // arm64: truncating, and saturating out of range.
            self.rate = (0.5 + 1.0 / (dt * rt_hz)) as u32;
            self.enabled = true;
        } else {
            self.rate = 1;
            self.enabled = false;
        }
    }

    /// `GetRateHz()` at time step `dt`: the rate the frame count gives
    /// back, not the one last written.
    #[must_use]
    pub fn rate_hz(&self, dt: f64) -> f64 {
        1.0 / (f64::from(self.rate) * dt)
    }
}

fn flag(b: bool) -> f64 {
    f64::from(u8::from(b))
}

impl Output {
    /// `FGOutput::Load(element)` at time step `dt`, without the file or
    /// socket: makes output type `n` (the next index), ties its properties
    /// (`SetIdx`) and sets its rate from the `rate` attribute (`Load`).
    /// Types JSBSim has no output type for fail the load, as in JSBSim.
    pub fn load(&mut self, el: &Element, b: &mut Binder<'_>, dt: f64) -> Result<(), Error> {
        let kind = el.attribute("type").to_ascii_uppercase();
        if !matches!(kind.as_str(), "CSV" | "TABULAR" | "SOCKET" | "FLIGHTGEAR") {
            if kind != "TERMINAL" && kind != "NONE" {
                tracing::error!("Unknown type of output specified in config file");
            }
            return Err(Error::Model {
                message: format!(
                    "{}: no output type for <output type=\"{}\">, so the aircraft does not load",
                    el.location(),
                    el.attribute("type")
                ),
            });
        }
        let idx = self.types.len();
        self.types.push(OutputType::default());
        let prefix = format!("simulation/output[{idx}]");
        b.property(
            &format!("{prefix}/log_rate_hz"),
            Kind::Double,
            move |m| {
                m.output
                    .types
                    .get(idx)
                    .map_or(0.0, |t| t.rate_hz(m.exec.dt))
            },
            move |m, v| {
                let dt = m.exec.dt;
                if let Some(t) = m.output.types.get_mut(idx) {
                    t.set_rate_hz(v, dt);
                }
            },
        )?;
        b.property(
            &format!("{prefix}/enabled"),
            Kind::Bool,
            move |m| m.output.types.get(idx).map_or(0.0, |t| flag(t.enabled)),
            move |m, v| {
                if let Some(t) = m.output.types.get_mut(idx) {
                    t.enabled = v != 0.0;
                }
            },
        )?;
        let rate = if el.has_attribute("rate") {
            el.attribute_as_number("rate")?
        } else {
            1.0
        };
        if let Some(t) = self.types.get_mut(idx) {
            t.set_rate_hz(rate, dt);
        }
        Ok(())
    }
}
