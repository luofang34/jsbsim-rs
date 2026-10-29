// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2013 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGWaypoint.{h,cpp}.
//
//! `<waypoint_heading>` and `<waypoint_distance>`: the initial geodesic
//! heading, or the geodesic distance, from a source to a target latitude
//! and longitude read from properties.

use std::f64::consts::FRAC_PI_2;

use super::component::{Base, Tick, model_error};
use super::families::{ComponentAt, Family};
use crate::Error;
use crate::fdm::LoadContext;
use crate::fdm::ModelEvaluator;
use crate::functions::PropertyValue;
use crate::math::Location;
use crate::units::{RADTODEG, feet_to_meters};
use crate::xml::Element;

#[cfg(test)]
mod tests;

/// The factor JSBSim applies to a coordinate whose `unit` is `DEG`: a
/// rounded degree-to-radian ratio, not `DEGTORAD`.
const DEG_UNIT: f64 = 0.017453293;

/// Which output a waypoint computes (`WaypointType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaypointType {
    /// `<waypoint_heading>`.
    Heading,
    /// `<waypoint_distance>`.
    Distance,
}

/// The output unit (`eUnit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Heading in radians, the heading default.
    Rad,
    /// Heading in degrees.
    Deg,
    /// Distance in feet, the distance default.
    Feet,
    /// Distance in metres.
    Meters,
}

/// One coordinate: its property and the factor taking it to radians.
#[derive(Debug, Clone)]
pub struct Coordinate {
    /// The property, possibly negated.
    pub value: PropertyValue,
    /// `DEG_UNIT` when `unit="DEG"`, else 1.
    pub unit: f64,
}

/// `FGWaypoint`.
#[derive(Debug, Clone)]
pub struct Waypoint {
    /// Heading or distance.
    pub kind: WaypointType,
    /// The output unit.
    pub unit: Unit,
    /// `target_latitude`.
    pub target_latitude: Coordinate,
    /// `target_longitude`.
    pub target_longitude: Coordinate,
    /// `source_latitude`.
    pub source_latitude: Coordinate,
    /// `source_longitude`.
    pub source_longitude: Coordinate,
    /// A copy of the initial condition's position when the component
    /// loaded; only its ellipse outlives a run, which sets its position.
    pub source: Location,
}

/// The coordinates one run reads, in radians.
#[derive(Debug, Clone, Copy)]
pub struct Coordinates {
    source_latitude: f64,
    source_longitude: f64,
    target_latitude: f64,
    target_longitude: f64,
}

/// A required coordinate element, or JSBSim's error naming `what`.
fn coordinate(
    el: &Element,
    tag: &str,
    what: &str,
    name: &str,
    ctx: &mut LoadContext<'_>,
) -> Result<Coordinate, Error> {
    let Some(coord_el) = el.find_element(tag) else {
        return Err(model_error(
            el,
            format!("{what} is required for waypoint component: {name}"),
        ));
    };
    let value = PropertyValue::new(coord_el.data_line(0), ctx.tree(), coord_el.location())?;
    let unit = if coord_el.attribute("unit") == "DEG" {
        DEG_UNIT
    } else {
        1.0
    };
    Ok(Coordinate { value, unit })
}

/// The component's `unit` attribute as `eUnit`.
fn output_unit(el: &Element, kind: WaypointType, name: &str) -> Result<Unit, Error> {
    let unit = el.attribute("unit");
    match (kind, unit) {
        (WaypointType::Heading, "") | (WaypointType::Heading, "RAD") => Ok(Unit::Rad),
        (WaypointType::Heading, "DEG") => Ok(Unit::Deg),
        (WaypointType::Heading, _) => Err(model_error(
            el,
            format!("Unknown unit {unit} in HEADING waypoint component, "),
        )),
        (WaypointType::Distance, "") | (WaypointType::Distance, "FT") => Ok(Unit::Feet),
        (WaypointType::Distance, "M") => Ok(Unit::Meters),
        (WaypointType::Distance, _) => Err(model_error(
            el,
            format!("Unknown unit {unit} in DISTANCE waypoint component, {name}"),
        )),
    }
}

/// JSBSim's run-time error for a latitude beyond +-90 degrees.
fn latitude_error(which: &str, name: &str) -> Error {
    Error::Model {
        message: format!(
            "{which} latitude in waypoint \"{name}\" must be less than or equal to 90 degrees.\n(is longitude being mistakenly supplied?)"
        ),
    }
}

fn read(c: &Coordinate, ev: &mut ModelEvaluator<'_>) -> Result<f64, Error> {
    Ok(ev.property(&c.value)? * c.unit)
}

impl Family for Waypoint {
    type Sample = Coordinates;

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        _at: ComponentAt,
    ) -> Result<Self, Error> {
        let kind = if el.name() == "waypoint_heading" {
            WaypointType::Heading
        } else {
            WaypointType::Distance
        };
        let source = ctx
            .ic_position
            .ok_or_else(|| model_error(el, "a waypoint loads only with an initial condition"))?;
        let name = base.name.as_str();
        let target_latitude = coordinate(el, "target_latitude", "Target latitude", name, ctx)?;
        let target_longitude = coordinate(el, "target_longitude", "Target longitude", name, ctx)?;
        let source_latitude = coordinate(el, "source_latitude", "Source latitude", name, ctx)?;
        let source_longitude = coordinate(el, "source_longitude", "Source longitude", name, ctx)?;
        let unit = output_unit(el, kind, name)?;
        base.bind(ctx)?;
        Ok(Self {
            kind,
            unit,
            target_latitude,
            target_longitude,
            source_latitude,
            source_longitude,
            source,
        })
    }

    fn sample(&self, base: &Base, ev: &mut ModelEvaluator<'_>) -> Result<Coordinates, Error> {
        let c = Coordinates {
            source_latitude: read(&self.source_latitude, ev)?,
            source_longitude: read(&self.source_longitude, ev)?,
            target_latitude: read(&self.target_latitude, ev)?,
            target_longitude: read(&self.target_longitude, ev)?,
        };
        if c.target_latitude.abs() > FRAC_PI_2 {
            return Err(latitude_error("Target", &base.name));
        }
        if c.source_latitude.abs() > FRAC_PI_2 {
            return Err(latitude_error("Source", &base.name));
        }
        Ok(c)
    }

    fn step(&mut self, base: &mut Base, c: Coordinates, tick: &Tick) {
        self.source
            .set_position_geodetic(c.source_longitude, c.source_latitude, 0.0);
        // Computed once here rather than by each read below.
        self.source.cache_derived();
        base.output = match self.kind {
            WaypointType::Heading => {
                let heading = self
                    .source
                    .heading_to(c.target_longitude, c.target_latitude);
                if self.unit == Unit::Deg {
                    heading * RADTODEG
                } else {
                    heading
                }
            }
            WaypointType::Distance => {
                let distance = self
                    .source
                    .distance_to(c.target_longitude, c.target_latitude);
                if self.unit == Unit::Meters {
                    feet_to_meters(distance)
                } else {
                    distance
                }
            }
        };
        base.clip(tick);
    }
}
