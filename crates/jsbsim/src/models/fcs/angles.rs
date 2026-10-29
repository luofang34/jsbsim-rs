// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2013 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGAngles.{h,cpp}.
//
//! `<angle>`: the signed smallest angle from a source angle to a target
//! angle (the "included angle to heading" algorithm).

#[cfg(test)]
mod tests;

use super::component::{Base, Tick, model_error};
use super::families::{ComponentAt, Family};
use crate::Error;
use crate::fdm::LoadContext;
use crate::fdm::ModelEvaluator;
use crate::properties::NodeId;
use crate::xml::Element;

/// JSBSim's rounded degrees-to-radians factor for `unit="DEG"` angles.
const DEG: f64 = 0.017453293;

/// `FGAngles`.
#[derive(Debug, Clone)]
pub struct Angles {
    /// The `<target_angle>` node; `None` when it did not exist at load.
    pub target: Option<NodeId>,
    /// The `<source_angle>` node; `None` when it did not exist at load.
    pub source: Option<NodeId>,
    /// The `<target_angle>` path, for the error a missing node gives.
    pub target_path: String,
    /// The `<source_angle>` path, for the error a missing node gives.
    pub source_path: String,
    /// Radians per unit of the target angle.
    pub target_unit: f64,
    /// Radians per unit of the source angle.
    pub source_unit: f64,
    /// Output units per radian.
    pub output_unit: f64,
}

/// `std::min(a, b)`: `b` only when `b < a`.
fn cpp_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// `std::max(a, b)`: `b` only when `a < b`.
fn cpp_max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// One angle element: its path, its node as `GetNode` finds it at load
/// (not created, not late bound) and its unit.
fn angle(
    el: &Element,
    name: &str,
    what: &str,
    base: &Base,
    ctx: &mut LoadContext<'_>,
) -> Result<(String, Option<NodeId>, f64), Error> {
    let Some(angle_el) = el.find_element(name) else {
        return Err(model_error(
            el,
            format!(
                "{what} angle is required for Angles component: {}",
                base.name
            ),
        ));
    };
    let path = el.find_element_value(name).to_owned();
    let node = ctx.tree().get(&path)?;
    let unit = if angle_el.has_attribute("unit") && angle_el.attribute("unit") == "DEG" {
        DEG
    } else {
        1.0
    };
    Ok((path, node, unit))
}

/// The node of an angle, or the error JSBSim's null dereference becomes.
fn read(
    node: Option<NodeId>,
    path: &str,
    base: &Base,
    ev: &mut ModelEvaluator<'_>,
) -> Result<f64, Error> {
    match node {
        Some(node) => ev.read_node(node),
        None => Err(Error::Model {
            message: format!(
                "Angles component {}: the property {path} did not exist when it loaded",
                base.name
            ),
        }),
    }
}

impl Family for Angles {
    /// The source angle, then the target angle, as read.
    type Sample = (f64, f64);

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        _at: ComponentAt,
    ) -> Result<Self, Error> {
        let (target_path, target, target_unit) = angle(el, "target_angle", "Target", base, ctx)?;
        let (source_path, source, source_unit) = angle(el, "source_angle", "Source", base, ctx)?;
        let output_unit = match el.attribute("unit") {
            "" | "RAD" => 1.0,
            "DEG" => 180.0 / std::f64::consts::PI,
            unit => {
                return Err(model_error(
                    el,
                    format!("Unknown unit {unit} in angle component, {}", base.name),
                ));
            }
        };
        base.bind(ctx)?;
        Ok(Self {
            target,
            source,
            target_path,
            source_path,
            target_unit,
            source_unit,
            output_unit,
        })
    }

    fn sample(&self, base: &Base, ev: &mut ModelEvaluator<'_>) -> Result<(f64, f64), Error> {
        let source = read(self.source, &self.source_path, base, ev)?;
        let target = read(self.target, &self.target_path, base, ev)?;
        Ok((source, target))
    }

    fn step(&mut self, base: &mut Base, (source, target): (f64, f64), tick: &Tick) {
        let source_angle = source * self.source_unit;
        let target_angle = target * self.target_unit;
        let (y1, x1) = libm::sincos(source_angle);
        let (y2, x2) = libm::sincos(target_angle);
        let x1x2_y1y2 = cpp_max(-1.0, cpp_min(x1 * x2 + y1 * y2, 1.0));
        let angle_to_heading_rad = libm::acos(x1x2_y1y2);
        let x1y2 = x1 * y2;
        let x2y1 = x2 * y1;
        base.output = if x1y2 >= x2y1 {
            angle_to_heading_rad * self.output_unit
        } else {
            -angle_to_heading_rad * self.output_unit
        };
        base.clip(tick);
    }
}

crate::snapshot::visit_fields! {
    Angles { target, source, target_unit, source_unit, output_unit }
    skip { target_path, source_path }
}
