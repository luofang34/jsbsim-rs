// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGMassBalance.cpp (Load, AddPointMass,
// ReadInertiaMatrix).
//
//! Reading the `<mass_balance>` element.

use super::{MassBalance, PointMass, Shape, bind};
use crate::Error;
use crate::fdm::Binder;
use crate::math::Matrix33;
use crate::xml::{Element, XmlError};

/// The optional child `name` converted to `unit`, 0 when absent.
fn optional(el: &Element, name: &str, unit: &str) -> Result<f64, XmlError> {
    match el.find_element(name) {
        Some(_) => el.find_element_value_as_number_convert_to(name, unit),
        None => Ok(0.0),
    }
}

/// `ReadInertiaMatrix`: `ixx`..`iyz` of `el`, structural frame, into the
/// body frame. The products of inertia are taken as negated integrals
/// unless `negated_crossproduct_inertia` is exactly `"false"`.
fn read_inertia_matrix(el: &Element) -> Result<Matrix33, XmlError> {
    let unit = "SLUG*FT2";
    let ixx = optional(el, "ixx", unit)?;
    let iyy = optional(el, "iyy", unit)?;
    let izz = optional(el, "izz", unit)?;
    let ixy = optional(el, "ixy", unit)?;
    let ixz = optional(el, "ixz", unit)?;
    let iyz = optional(el, "iyz", unit)?;
    Ok(if el.attribute("negated_crossproduct_inertia") == "false" {
        Matrix33::new(ixx, ixy, -ixz, ixy, iyy, iyz, -ixz, iyz, izz)
    } else {
        Matrix33::new(ixx, -ixy, ixz, -ixy, iyy, -iyz, ixz, -iyz, izz)
    })
}

impl MassBalance {
    /// `Load(<mass_balance>)`. `el` is the element after the aircraft
    /// loader has merged any `file=` include into it; its `<function>` and
    /// `<property>` children are the loader's. Point mass properties are
    /// tied through `b` as each point mass is added.
    pub fn load(&mut self, el: &Element, b: &mut Binder<'_>) -> Result<(), Error> {
        self.set_aircraft_base_inertias(read_inertia_matrix(el)?);
        if el.find_element("emptywt").is_some() {
            self.empty_weight = el.find_element_value_as_number_convert_to("emptywt", "LBS")?;
        }
        for location in el.find_elements("location") {
            if location.attribute("name") == "CG" {
                self.base_xyz_cg = location.find_element_triplet_convert_to("IN")?;
            }
        }
        for pm in el.find_elements("pointmass") {
            self.add_point_mass(pm, b)?;
        }
        self.update_weight();
        tracing::debug!(
            "Mass and Balance: base inertia {:?}, empty weight {} lbm, CG {:?}, {} point masses",
            self.base_j,
            self.empty_weight,
            self.base_xyz_cg,
            self.point_masses.len()
        );
        sanity_check(self);
        Ok(())
    }

    /// `AddPointMass`.
    pub fn add_point_mass(&mut self, el: &Element, b: &mut Binder<'_>) -> Result<(), Error> {
        let Some(loc_element) = el.find_element("location") else {
            tracing::error!(
                "{}: Pointmass {} has no location.",
                el.location(),
                el.attribute("name")
            );
            return Err(XmlError::MissingElement {
                parent: el.name().to_owned(),
                name: "location".to_owned(),
                at: el.location(),
            }
            .into());
        };
        let w = el.find_element_value_as_number_convert_to("weight", "LBS")?;
        let xyz = loc_element.find_element_triplet_convert_to("IN")?;
        let mut pm = PointMass::new(w, xyz);
        pm.name = el.attribute("name").to_owned();

        if let Some(form) = el.find_element("form") {
            let radius = optional(form, "radius", "FT")?;
            let length = optional(form, "length", "FT")?;
            let shape = match form.attribute("shape") {
                "tube" => Some((Shape::Tube, true)),
                "cylinder" => Some((Shape::Cylinder, true)),
                "sphere" => Some((Shape::Sphere, false)),
                "ball" => Some((Shape::Ball, false)),
                _ => None,
            };
            if let Some((shape, has_length)) = shape {
                pm.shape = shape;
                pm.radius = radius;
                if has_length {
                    pm.length = length;
                }
                pm.calculate_shape_inertia();
            }
        } else {
            pm.shape = Shape::Unspecified;
            pm.inertia = read_inertia_matrix(el)?;
        }

        bind::bind_point_mass(b, self.point_masses.len())?;
        self.point_masses.push(pm);
        Ok(())
    }
}

/// The out-of-bounds warnings of JSBSim's sanity-checking debug level.
fn sanity_check(mb: &MassBalance) {
    let out = |v: f64| v <= 0.0 || v > 1e9;
    if out(mb.empty_weight) {
        tracing::debug!(
            "MassBalance::EmptyWeight out of bounds: {}",
            mb.empty_weight
        );
    }
    if out(mb.weight) {
        tracing::debug!("MassBalance::Weight out of bounds: {}", mb.weight);
    }
    if out(mb.mass) {
        tracing::debug!("MassBalance::Mass out of bounds: {}", mb.mass);
    }
}
