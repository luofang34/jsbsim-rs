// SPDX-License-Identifier: LGPL-2.1-or-later
// FGXMLElement: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/input_output/FGXMLElement.cpp (unit-converting
// accessors).
//
//! Reading numbers with a `unit` attribute converted to a target unit.
//!
//! JSBSim applies random dispersions here when the environment variable
//! `JSBSIM_DISPERSE` is 1. The core reads no environment, so dispersions are
//! never applied, which is JSBSim's behaviour without that variable.

use std::f64::consts::PI;
use std::sync::OnceLock;

use super::{Element, XmlError};
use crate::math::ColumnVector3;
use crate::units::UnitTable;

fn table() -> &'static UnitTable {
    static TABLE: OnceLock<UnitTable> = OnceLock::new();
    TABLE.get_or_init(UnitTable::new)
}

impl Element {
    /// The factor from `supplied` to `target`, checked as JSBSim checks it;
    /// `None` when no unit is supplied.
    fn conversion(&self, supplied: &str, target: &str) -> Result<Option<f64>, XmlError> {
        if supplied.is_empty() {
            return Ok(None);
        }
        if !table().knows(supplied) {
            return Err(XmlError::UnknownUnit {
                unit: supplied.to_owned(),
                at: self.location(),
            });
        }
        table()
            .factor(supplied, target)
            .map(Some)
            .ok_or_else(|| XmlError::Inconvertible {
                from: supplied.to_owned(),
                to: target.to_owned(),
                at: self.location(),
            })
    }

    /// `FindElementValueAsNumberConvertTo(name, target)`: the child's value
    /// converted from its `unit` attribute, if any, to `target`.
    pub fn find_element_value_as_number_convert_to(
        &self,
        name: &str,
        target: &str,
    ) -> Result<f64, XmlError> {
        let child = self.require(name)?;
        let supplied = child.attribute("unit");
        let factor = child.conversion(supplied, target)?;
        let mut value = child.data_as_number()?;
        child.warn_out_of_range(supplied, value);
        if let Some(factor) = factor {
            value *= factor;
        }
        child.warn_out_of_range(target, value);
        Ok(value)
    }

    /// `FindElementValueAsNumberConvertFromTo(name, supplied, target)`.
    pub fn find_element_value_as_number_convert_from_to(
        &self,
        name: &str,
        supplied: &str,
        target: &str,
    ) -> Result<f64, XmlError> {
        let child = self.require(name)?;
        let factor = child.conversion(supplied, target)?;
        let value = child.data_as_number()?;
        Ok(factor.map_or(value, |f| value * f))
    }

    /// `FindElementTripletConvertTo(target)`: children `x`/`roll`,
    /// `y`/`pitch` and `z`/`yaw`, converted from this element's `unit`;
    /// a missing child reads as 0.
    pub fn find_element_triplet_convert_to(&self, target: &str) -> Result<ColumnVector3, XmlError> {
        let supplied = self.attribute("unit");
        let factor = self.conversion(supplied, target)?;
        let mut triplet = ColumnVector3::zero();
        for (i, (name, alias)) in [("x", "roll"), ("y", "pitch"), ("z", "yaw")]
            .into_iter()
            .enumerate()
        {
            if let Some(item) = self.find_element(name).or_else(|| self.find_element(alias)) {
                let value = item.data_as_number()?;
                triplet[i + 1] = factor.map_or(value, |f| value * f);
            }
        }
        Ok(triplet)
    }

    fn warn_out_of_range(&self, unit: &str, value: f64) {
        if unit == "RAD" && value.abs() > 2.0 * PI {
            tracing::error!(
                "{}: {} value {value} RAD is outside the range [ -2*M_PI RAD ; +2*M_PI RAD ]",
                self.location(),
                self.name
            );
        }
        if unit == "DEG" && value.abs() > 360.0 {
            tracing::error!(
                "{}: {} value {value} DEG is outside the range [ -360 DEG ; +360 DEG ]",
                self.location(),
                self.name
            );
        }
    }
}
