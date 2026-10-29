// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2009 Matthew Chave.
// Derived from JSBSim src/models/flight_control/FGMagnetometer.{h,cpp}.
//
//! `<magnetometer>`: the Earth's magnetic field from JSBSim's bundled
//! WMM2005 model ([`magvar`]) at the vehicle's geodetic position, in the
//! body frame along a sensor axis, degraded as a [`Signal`].
//!
//! JSBSim dates the model by the wall clock when the magnetometer loads.
//! The core reads no clock: [`ExecState::wall_clock`] stands for it, and
//! [`construct`] finishes the constructor once the channels are loaded,
//! since the constructor also evaluates the model at the position the
//! vehicle has then.

pub mod magvar;
#[cfg(test)]
mod tests;

use super::component::{Base, Tick, model_error};
use super::families::{ComponentAt, Family};
use super::sensor::Signal;
use super::sensor::orientation::Orientation;
use super::{Fcs, Kind};
use crate::Error;
use crate::fdm::ModelEvaluator;
use crate::fdm::{ExecState, LoadContext, Models};
use crate::math::{ColumnVector3, Matrix33};
use crate::models::propagate::Propagate;
use crate::units::FTTOM;
use crate::xml::Element;

/// `INERTIAL_UPDATE_RATE`: the field is evaluated every this many runs.
const INERTIAL_UPDATE_RATE: u32 = 1000;

/// `FGMagnetometer`.
#[derive(Debug, Clone)]
pub struct Magnetometer {
    /// The degradation.
    pub signal: Signal,
    /// Axis and frame.
    pub orientation: Orientation,
    /// `<location>`, structural frame, in; JSBSim reads but never uses it.
    pub location: ColumnVector3,
    /// North, east and down field, nT, as last evaluated.
    pub field: ColumnVector3,
    /// Julian day the model is evaluated at.
    pub date: i64,
    /// Runs counted towards the next evaluation.
    pub counter: u32,
    /// Whether [`construct`] has run for it.
    pub constructed: bool,
}

/// What one run reads: the field when this run evaluates it, and the
/// local-to-body rotation.
#[derive(Debug, Clone, Copy)]
pub struct Sample {
    field: Option<ColumnVector3>,
    tl2b: Matrix33,
}

/// The model's north, east and down field at the vehicle's position.
fn field_at(propagate: &Propagate, date: i64) -> ColumnVector3 {
    let location = &propagate.state.location;
    let lat = location.geod_latitude_rad();
    let lon = location.longitude();
    let alt = location.geod_altitude() * FTTOM * 0.001;
    let [_, _, _, x, y, z] = magvar::calc_magvar(lat, lon, alt, date);
    ColumnVector3::new(x, y, z)
}

/// The civil UTC date of `seconds` since the Unix epoch, as `gmtime`
/// gives it: years since 1900, the zero-based month and the day.
fn gmtime_date(seconds: i64) -> (i64, i64, i64) {
    // Howard Hinnant's civil_from_days.
    let z = seconds.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year - 1900, month - 1, day)
}

/// The constructor's date: `yymmdd_to_julian_days` of `gmtime`'s year
/// since 1900, zero-based month and day, as JSBSim passes them.
fn date(exec: &ExecState) -> i64 {
    let (year, month, day) = gmtime_date(exec.wall_clock);
    magvar::yymmdd_to_julian_days(year, month, day)
}

impl Magnetometer {
    /// `updateInertialMag`'s schedule: whether this run evaluates.
    fn due(&self) -> bool {
        self.counter.is_multiple_of(INERTIAL_UPDATE_RATE)
    }
}

/// The rest of each newly loaded magnetometer's constructor: its date and
/// its first `updateInertialMag`, at the vehicle's position now.
pub fn construct(fcs: &mut Fcs, propagate: &Propagate, exec: &ExecState) {
    let date = date(exec);
    for channel in &mut fcs.channels {
        for component in &mut channel.components {
            if let Kind::Magnetometer(m) = &mut component.kind
                && !m.constructed
            {
                m.date = date;
                if m.due() {
                    m.field = field_at(propagate, date);
                }
                m.counter = m.counter.wrapping_add(1);
                m.constructed = true;
            }
        }
    }
}

impl Family for Magnetometer {
    type Sample = Sample;

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        at: ComponentAt,
    ) -> Result<Self, Error> {
        let signal = Signal::load(el, base, ctx, at)?;
        let orientation = Orientation::load(el, ctx)?;
        let Some(location_el) = el.find_element("location") else {
            return Err(model_error(el, "No location given for magnetometer."));
        };
        Ok(Self {
            signal,
            orientation,
            location: location_el.find_element_triplet_convert_to("IN")?,
            field: ColumnVector3::zero(),
            date: 0,
            counter: 0,
            constructed: false,
        })
    }

    fn sample(&self, base: &Base, _ev: &mut ModelEvaluator<'_>) -> Result<Sample, Error> {
        Err(super::sensor::needs_models(base))
    }

    fn sample_models(
        &self,
        _base: &Base,
        models: &Models,
        _ev: &mut ModelEvaluator<'_>,
    ) -> Result<Sample, Error> {
        let p = &models.propagate;
        Ok(Sample {
            field: self.due().then(|| field_at(p, self.date)),
            tl2b: p.frames.tl2b,
        })
    }

    fn step(&mut self, base: &mut Base, s: Sample, tick: &Tick) {
        self.counter = self.counter.wrapping_add(1);
        if let Some(field) = s.field {
            self.field = field;
        }
        base.input = self.orientation.measure(s.tl2b * self.field);
        self.signal.process(base, tick);
    }

    fn reset(&mut self, base: &mut Base) {
        self.signal.reset(base);
        self.counter = 0;
    }
}
