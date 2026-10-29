// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGTank.cpp (the constructor).
//
//! Reading a `<tank>` element.

use super::{Grain, GrainType, Initial, Tank, TankType, fuel_density};
use crate::Error;
use crate::math::ColumnVector3;
use crate::units::{LBTOSLUG, fahrenheit_to_celsius};
use crate::xml::{Element, XmlError};

/// The child `name` converted to `unit`, or `default` when absent.
fn optional(el: &Element, name: &str, unit: &str, default: f64) -> Result<f64, XmlError> {
    match el.find_element(name) {
        Some(_) => el.find_element_value_as_number_convert_to(name, unit),
        None => Ok(default),
    }
}

/// What JSBSim throws as an `XMLLogException` while it reads `el`.
fn refused(el: &Element, message: String) -> Error {
    Error::Model {
        message: format!("{}: {message}", el.location()),
    }
}

impl Tank {
    /// `FGTank(exec, el, tank_number)` up to `bind`, which the caller does
    /// once the tank is in place.
    pub fn load(el: &Element, number: usize) -> Result<Self, Error> {
        let mut tank = Self::read(el, number)?;
        tank.set_priority(tank.priority);
        tank.check_capacity(el)?;
        tank.pct_full = 100.0 * tank.contents / tank.capacity;
        if let Some(grain) = el.find_element("grain_config") {
            tank.load_grain(el, grain)?;
        }
        if tank.grain_density_fault() {
            return Err(refused(
                el,
                "Solid propellant grain density is zero!".to_owned(),
            ));
        }
        tank.calculate_inertias();
        if tank.temperature != -9999.0 {
            tank.temperature = fahrenheit_to_celsius(tank.temperature);
            tank.initial.temperature = tank.temperature;
        }
        tank.area = 40.0 * crate::elementary::pow(tank.capacity / 1975.0, 0.666666667);
        if el.find_element("type").is_some() {
            let name = el.find_element_value("type");
            if !name.is_empty() {
                tank.density = fuel_density(name);
            }
        }
        Ok(tank)
    }

    /// The constructor's reads of `el`, with JSBSim's defaults.
    fn read(el: &Element, number: usize) -> Result<Self, Error> {
        let tank_type = match el.attribute("type") {
            "FUEL" => TankType::Fuel,
            "OXIDIZER" => TankType::Oxidizer,
            _ => TankType::Unknown,
        };
        let xyz = match el.find_element("location") {
            Some(location) => location.find_element_triplet_convert_to("IN")?,
            None => {
                tracing::error!("{}: No location found for this tank.", el.location());
                ColumnVector3::zero()
            }
        };
        let xyz_drain = match el.find_element("drain_location") {
            Some(drain) => drain.find_element_triplet_convert_to("IN")?,
            None => xyz,
        };
        let contents = optional(el, "contents", "LBS", 0.0)?;
        let temperature = match el.find_element("temperature") {
            Some(_) => el.find_element_value_as_number("temperature")?,
            None => -9999.0,
        };
        let standpipe = optional(el, "standpipe", "LBS", 0.0)?;
        let priority = match el.find_element("priority") {
            Some(_) => el.find_element_value_as_number("priority")? as i32,
            None => 1,
        };
        Ok(Self {
            tank_type,
            grain: Grain {
                kind: GrainType::Unknown,
                length: 0.0,
                inner_radius: 0.0,
                volume: 0.0,
            },
            number,
            name: el.attribute("name").to_owned(),
            xyz,
            xyz_drain,
            capacity: optional(el, "capacity", "LBS", 0.00001)?,
            unusable_volume: optional(el, "unusable-volume", "GAL", 0.0)?,
            radius: optional(el, "radius", "IN", 0.0)?,
            density: optional(el, "density", "LBS/GAL", 6.6)?,
            ixx: 0.0,
            iyy: 0.0,
            izz: 0.0,
            inertia_factor: match el.find_element("inertia_factor") {
                Some(_) => el.find_element_value_as_number("inertia_factor")?,
                None => 1.0,
            },
            pct_full: 0.0,
            contents,
            area: 1.0,
            temperature,
            standpipe,
            external_flow: 0.0,
            selected: false,
            priority,
            initial: Initial {
                contents,
                temperature,
                standpipe,
                priority,
            },
        })
    }

    /// The capacity and contents checks of the constructor.
    fn check_capacity(&mut self, el: &Element) -> Result<(), Error> {
        if self.capacity == 0.0 {
            tracing::warn!(
                "{}: Tank capacity must not be zero. Reset to 0.00001 lbs!",
                el.location()
            );
            self.capacity = 0.00001;
            self.contents = 0.0;
        }
        let n = self.number;
        if self.capacity <= self.unusable() {
            return Err(refused(
                el,
                format!(
                    "Tank capacity ({} lbs) is lower than the amount of unusable fuel ({} lbs) for tank {n}! Did you accidentally swap unusable and capacity?",
                    self.capacity,
                    self.unusable()
                ),
            ));
        }
        if self.contents > self.capacity {
            return Err(refused(
                el,
                format!(
                    "Tank content ({} lbs) is greater than tank capacity ({} lbs) for tank {n}! Did you accidentally swap contents and capacity?",
                    self.contents, self.capacity
                ),
            ));
        }
        if self.contents < self.unusable() {
            tracing::error!(
                "{}: Tank content ({} lbs) is lower than the amount of unusable fuel ({} lbs) for tank {n}",
                el.location(),
                self.contents,
                self.unusable()
            );
        }
        Ok(())
    }

    /// The `<grain_config>` of a solid propellant tank, which sets its
    /// geometry and replaces the density with slugs per cubic inch.
    fn load_grain(&mut self, el: &Element, grain: &Element) -> Result<(), Error> {
        let kind = match grain.attribute("type") {
            "CYLINDRICAL" => GrainType::Cylindrical,
            "ENDBURNING" => GrainType::EndBurning,
            "FUNCTION" => {
                return Err(Error::Unsupported {
                    construct: "<grain_config type=\"FUNCTION\"> (inertia functions)".to_owned(),
                    path: grain.location().to_string(),
                });
            }
            _ => {
                tracing::error!("{}: Unknown propellant grain type specified", el.location());
                GrainType::Unknown
            }
        };
        let g = &mut self.grain;
        g.kind = kind;
        g.length = optional(grain, "length", "IN", 0.0)?;
        g.inner_radius = optional(grain, "bore_diameter", "IN", 0.0)? / 2.0;
        let pi = std::f64::consts::PI;
        g.volume = match kind {
            GrainType::Cylindrical => {
                if self.radius <= g.inner_radius {
                    return Err(refused(
                        grain,
                        "The bore diameter should be smaller than the total grain diameter!"
                            .to_owned(),
                    ));
                }
                pi * g.length * (self.radius * self.radius - g.inner_radius * g.inner_radius)
            }
            GrainType::EndBurning => pi * g.length * self.radius * self.radius,
            GrainType::Unknown => {
                return Err(refused(
                    el,
                    "Unknown grain type found in this rocket engine definition.".to_owned(),
                ));
            }
        };
        self.density = (self.capacity * LBTOSLUG) / g.volume;
        Ok(())
    }
}
