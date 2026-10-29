// SPDX-License-Identifier: LGPL-2.1-or-later
// FGGasCell, FGBallonet: Copyright (C) 2006 - 2013 Anders Gidenstam.
// Derived from JSBSim src/models/FGGasCell.cpp (the shape, envelope and
// inertia code FGGasCell and FGBallonet share).
//
//! The extruded-ellipsoid shape of a gas cell or ballonet and the envelope
//! parameters both read from XML.

use std::f64::consts::PI;

use crate::Error;
use crate::math::ColumnVector3;
use crate::xml::Element;

/// Radii and widths of an extruded ellipsoid, ft.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Shape {
    /// `Xradius`.
    pub x_radius: f64,
    /// `Yradius`.
    pub y_radius: f64,
    /// `Zradius`.
    pub z_radius: f64,
    /// `Xwidth`.
    pub x_width: f64,
    /// `Ywidth`.
    pub y_width: f64,
    /// `Zwidth`.
    pub z_width: f64,
}

/// What the XML gives besides the shape.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Envelope {
    /// Location, structural frame, in.
    pub xyz: ColumnVector3,
    /// The shape.
    pub shape: Shape,
    /// `MaxVolume`, ft^3.
    pub max_volume: f64,
    /// `MaxOverpressure`, psf.
    pub max_overpressure: f64,
    /// Initial `Volume` from `<fullness>`, ft^3; 0 when absent.
    pub volume: f64,
    /// `ValveCoefficient`, ft^4·s/slug.
    pub valve_coefficient: f64,
}

impl Shape {
    fn is_ellipsoid(&self) -> bool {
        self.x_radius != 0.0
            && self.y_radius != 0.0
            && self.z_radius != 0.0
            && self.x_width == 0.0
            && self.y_width == 0.0
            && self.z_width == 0.0
    }

    fn is_cylinder(&self) -> bool {
        self.x_radius == 0.0
            && self.y_radius != 0.0
            && self.z_radius != 0.0
            && self.x_width != 0.0
            && self.y_width == 0.0
            && self.z_width == 0.0
    }

    /// The volume, ft^3; shapes other than an ellipsoid or an x-extruded
    /// cylinder get JSBSim's general formula with a warning.
    fn max_volume(&self, el: &Element) -> f64 {
        let (xr, yr, zr) = (self.x_radius, self.y_radius, self.z_radius);
        let (xw, yw, zw) = (self.x_width, self.y_width, self.z_width);
        if self.is_ellipsoid() {
            4.0 * PI * xr * yr * zr / 3.0
        } else if self.is_cylinder() {
            PI * yr * zr * xw
        } else {
            tracing::warn!("{}: Unsupported gas cell shape.", el.location());
            4.0 * PI * xr * yr * zr / 3.0
                + PI * yr * zr * xw
                + PI * xr * zr * yw
                + PI * xr * yr * zw
                + 2.0 * xr * yw * zw
                + 2.0 * yr * xw * zw
                + 2.0 * zr * xw * yw
                + xw * yw * zw
        }
    }

    /// `Ixx`, `Iyy`, `Izz` of `mass` slugs spread uniformly over the
    /// shape, about its centre; zero (a point mass) for shapes without a
    /// formula.
    #[must_use]
    pub fn inertia(&self, mass: f64) -> (f64, f64, f64) {
        let (xr, yr, zr, xw) = (self.x_radius, self.y_radius, self.z_radius, self.x_width);
        if self.is_ellipsoid() {
            (
                (1.0 / 5.0) * mass * (yr * yr + zr * zr),
                (1.0 / 5.0) * mass * (xr * xr + zr * zr),
                (1.0 / 5.0) * mass * (xr * xr + yr * yr),
            )
        } else if self.is_cylinder() {
            let iyy = (1.0 / 4.0) * mass * yr * zr + (1.0 / 12.0) * mass * xw * xw;
            ((1.0 / 2.0) * mass * yr * zr, iyy, iyy)
        } else {
            (0.0, 0.0, 0.0)
        }
    }
}

fn optional_ft(el: &Element, name: &str) -> Result<f64, Error> {
    match el.find_element(name) {
        Some(_) => Ok(el.find_element_value_as_number_convert_to(name, "FT")?),
        None => Ok(0.0),
    }
}

impl Envelope {
    /// The location, shape and envelope of a `<gas_cell>` or `<ballonet>`
    /// (`what` names it in errors).
    pub fn load(el: &Element, what: &str) -> Result<Self, Error> {
        let Some(location) = el.find_element("location") else {
            return Err(Error::Model {
                message: format!(
                    "{}: Fatal Error: No location found for this {what}.",
                    el.location()
                ),
            });
        };
        let xyz = location.find_element_triplet_convert_to("IN")?;
        let has = |a: &str, b: &str| el.find_element(a).is_some() || el.find_element(b).is_some();
        if !(has("x_radius", "x_width") && has("y_radius", "y_width") && has("z_radius", "z_width"))
        {
            let message = match what {
                "gas cell" => format!("{}: Gas cell shape must be given.", el.location()),
                _ => format!(
                    "{}: Fatal Error: Ballonet shape must be given.",
                    el.location()
                ),
            };
            return Err(Error::Model { message });
        }
        let shape = Shape {
            x_radius: optional_ft(el, "x_radius")?,
            y_radius: optional_ft(el, "y_radius")?,
            z_radius: optional_ft(el, "z_radius")?,
            x_width: optional_ft(el, "x_width")?,
            y_width: optional_ft(el, "y_width")?,
            z_width: optional_ft(el, "z_width")?,
        };
        let max_volume = shape.max_volume(el);
        let mut envelope = Self {
            xyz,
            shape,
            max_volume,
            ..Self::default()
        };
        if el.find_element("max_overpressure").is_some() {
            envelope.max_overpressure =
                el.find_element_value_as_number_convert_to("max_overpressure", "LBS/FT2")?;
        }
        if el.find_element("fullness").is_some() {
            let fullness = el.find_element_value_as_number("fullness")?;
            if 0.0 <= fullness {
                envelope.volume = fullness * max_volume;
            } else {
                tracing::warn!("{}: Invalid initial {what} fullness value.", el.location());
            }
        }
        if el.find_element("valve_coefficient").is_some() {
            let c =
                el.find_element_value_as_number_convert_to("valve_coefficient", "FT4*SEC/SLUG")?;
            envelope.valve_coefficient = c_max(c, 0.0);
        }
        Ok(envelope)
    }

    /// The initial `(Contents, Pressure)` at `temperature` and
    /// `pressure`: the fullness volume clipped to what the overpressure
    /// allows, or the whole volume at ambient pressure.
    #[must_use]
    pub fn initial_contents(&self, r: f64, temperature: f64, pressure: f64) -> (f64, f64) {
        if self.volume != 0.0 {
            let contents = pressure * self.volume / (r * temperature);
            let ideal_pressure = contents * r * temperature / self.max_volume;
            if ideal_pressure > pressure + self.max_overpressure {
                (
                    (pressure + self.max_overpressure) * self.max_volume / (r * temperature),
                    pressure + self.max_overpressure,
                )
            } else {
                (contents, c_max(ideal_pressure, pressure))
            }
        } else {
            (pressure * self.max_volume / (r * temperature), pressure)
        }
    }
}

/// `std::max(a, b)`: `b` only when `a < b`, so a NaN `a` is kept.
#[must_use]
pub fn c_max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// `std::min(a, b)`: `b` only when `b < a`.
#[must_use]
pub fn c_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

crate::snapshot::visit_fields! { Shape { x_radius, y_radius, z_radius, x_width, y_width, z_width } }
crate::snapshot::visit_fields! {
    Envelope { xyz, shape, max_volume, max_overpressure, volume, valve_coefficient }
}
