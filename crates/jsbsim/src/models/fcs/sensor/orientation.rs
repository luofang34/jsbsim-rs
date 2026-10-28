// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2009 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGSensorOrientation.h.
//
//! `FGSensorOrientation`: the axis an inertial sensor measures along and
//! the rotation from the body frame into the sensor's frame.

use crate::Error;
use crate::fdm::LoadContext;
use crate::math::{Axis, ColumnVector3, Matrix33};
use crate::models::fcs::component::model_error;
use crate::xml::Element;

/// `FGSensorOrientation`.
#[derive(Debug, Clone, Copy)]
pub struct Orientation {
    /// `mT`: body frame to sensor frame.
    pub transform: Matrix33,
    /// The measured axis.
    pub axis: Axis,
}

impl Orientation {
    /// `FGSensorOrientation(element)`: `<orientation>` (roll, pitch, yaw)
    /// and `<axis>`, X when missing or unknown.
    pub fn load(el: &Element, ctx: &LoadContext<'_>) -> Result<Self, Error> {
        let orient = match el.find_element("orientation") {
            Some(o) => o.find_element_triplet_convert_to("RAD")?,
            None => ColumnVector3::zero(),
        };
        let axis = match el.find_element("axis").map(|a| a.data_line(0)) {
            Some("X" | "x") => Axis::X,
            Some("Y" | "y") => Axis::Y,
            Some("Z" | "z") => Axis::Z,
            _ => {
                let message = "  Incorrect/no axis specified for this sensor; assuming X axis";
                if ctx.options.strict {
                    return Err(model_error(el, message));
                }
                tracing::warn!("{}: {message}", el.location());
                Axis::X
            }
        };
        Ok(Self {
            transform: transform(orient),
            axis,
        })
    }

    /// The component along the axis of `v` in the body frame.
    #[must_use]
    pub fn measure(&self, v: ColumnVector3) -> f64 {
        (self.transform * v)[self.axis]
    }
}

/// `CalculateTransformMatrix` of roll, pitch and yaw.
fn transform(orient: ColumnVector3) -> Matrix33 {
    let (cp, sp) = (libm::cos(orient[Axis::Y]), libm::sin(orient[Axis::Y]));
    let (cr, sr) = (libm::cos(orient[Axis::X]), libm::sin(orient[Axis::X]));
    let (cy, sy) = (libm::cos(orient[Axis::Z]), libm::sin(orient[Axis::Z]));
    Matrix33::new(
        cp * cy,
        cp * sy,
        -sp,
        sr * sp * cy - cr * sy,
        sr * sp * sy + cr * cy,
        sr * cp,
        cr * sp * cy + sr * sy,
        cr * sp * sy - sr * cy,
        cr * cp,
    )
}
