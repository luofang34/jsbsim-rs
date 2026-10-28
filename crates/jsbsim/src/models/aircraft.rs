// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGAircraft.{h,cpp}.
//
//! Aircraft metrics and the sum of all forces and moments.

use super::base::ModelBase;
use crate::Error;
use crate::fdm::{Binder, Kind};
use crate::math::{ColumnVector3, E_X, E_Y, E_Z};
use crate::units::RADTODEG;
use crate::xml::Element;

/// Forces and moments of every model (`FGAircraft::Inputs`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Inputs {
    /// Aerodynamic force and moment.
    pub aero: (ColumnVector3, ColumnVector3),
    /// Propulsion force and moment.
    pub propulsion: (ColumnVector3, ColumnVector3),
    /// Ground reaction force and moment.
    pub ground: (ColumnVector3, ColumnVector3),
    /// External reaction force and moment.
    pub external: (ColumnVector3, ColumnVector3),
    /// Buoyant force and moment.
    pub buoyant: (ColumnVector3, ColumnVector3),
}

/// `FGAircraft`.
#[derive(Debug, Clone, Default)]
pub struct Aircraft {
    /// Scheduling.
    pub base: ModelBase,
    /// Wing area, ft^2.
    pub wing_area: f64,
    /// Wing span, ft.
    pub wing_span: f64,
    /// Mean aerodynamic chord, ft.
    pub cbar: f64,
    /// Wing incidence, rad.
    pub wing_incidence: f64,
    /// Horizontal tail area, ft^2.
    pub htail_area: f64,
    /// Horizontal tail arm, ft.
    pub htail_arm: f64,
    /// Vertical tail area, ft^2.
    pub vtail_area: f64,
    /// Vertical tail arm, ft.
    pub vtail_arm: f64,
    /// Normalised tail arms and volumes.
    pub lbarh: f64,
    /// See `lbarh`.
    pub lbarv: f64,
    /// See `lbarh`.
    pub vbarh: f64,
    /// See `lbarh`.
    pub vbarv: f64,
    /// Aerodynamic reference point, structural frame, in.
    pub xyz_rp: ColumnVector3,
    /// Eye point, structural frame, in.
    pub xyz_ep: ColumnVector3,
    /// Visual reference point, structural frame, in.
    pub xyz_vrp: ColumnVector3,
    /// Total force, body frame, lbs.
    pub forces: ColumnVector3,
    /// Total moment, body frame, lbs·ft.
    pub moments: ColumnVector3,
}

impl Aircraft {
    /// `InitModel`.
    pub fn init_model(&mut self) {
        self.base.init();
        self.forces = ColumnVector3::zero();
        self.moments = ColumnVector3::zero();
    }

    /// `Run`, between the pre- and post-functions the executive caches.
    pub fn run(&mut self, input: &Inputs) {
        self.forces = input.aero.0;
        self.forces += input.propulsion.0;
        self.forces += input.ground.0;
        self.forces += input.external.0;
        self.forces += input.buoyant.0;
        self.moments = input.aero.1;
        self.moments += input.propulsion.1;
        self.moments += input.ground.1;
        self.moments += input.external.1;
        self.moments += input.buoyant.1;
    }

    /// `Load(<metrics>)`.
    pub fn load(&mut self, el: &Element) -> Result<(), Error> {
        let value = |name: &str, unit: &str| -> Result<Option<f64>, Error> {
            match el.find_element(name) {
                Some(_) => Ok(Some(
                    el.find_element_value_as_number_convert_to(name, unit)?,
                )),
                None => Ok(None),
            }
        };
        macro_rules! read {
            ($field:ident, $name:literal, $unit:literal) => {
                if let Some(v) = value($name, $unit)? {
                    self.$field = v;
                }
            };
        }
        read!(wing_area, "wingarea", "FT2");
        read!(wing_span, "wingspan", "FT");
        read!(cbar, "chord", "FT");
        read!(wing_incidence, "wing_incidence", "RAD");
        read!(htail_area, "htailarea", "FT2");
        read!(htail_arm, "htailarm", "FT");
        read!(vtail_area, "vtailarea", "FT2");
        read!(vtail_arm, "vtailarm", "FT");
        for location in el.find_elements("location") {
            match location.attribute("name") {
                "AERORP" => self.xyz_rp = location.find_element_triplet_convert_to("IN")?,
                "EYEPOINT" => self.xyz_ep = location.find_element_triplet_convert_to("IN")?,
                "VRP" => self.xyz_vrp = location.find_element_triplet_convert_to("IN")?,
                _ => {}
            }
        }
        if self.cbar != 0.0 {
            self.lbarh = self.htail_arm / self.cbar;
            self.lbarv = self.vtail_arm / self.cbar;
            if self.wing_area != 0.0 {
                self.vbarh = self.htail_arm * self.htail_area / (self.cbar * self.wing_area);
                self.vbarv = self.vtail_arm * self.vtail_area / (self.wing_span * self.wing_area);
            }
        }
        Ok(())
    }
}

/// `FGAircraft::bind`.
pub fn bind(b: &mut Binder<'_>) -> Result<(), Error> {
    b.property(
        "metrics/Sw-sqft",
        Kind::Double,
        |m| m.aircraft.wing_area,
        |m, v| m.aircraft.wing_area = v,
    )?;
    b.getter("metrics/bw-ft", |m| m.aircraft.wing_span)?;
    b.getter("metrics/cbarw-ft", |m| m.aircraft.cbar)?;
    b.getter("metrics/iw-rad", |m| m.aircraft.wing_incidence)?;
    b.getter("metrics/iw-deg", |m| m.aircraft.wing_incidence * RADTODEG)?;
    b.getter("metrics/Sh-sqft", |m| m.aircraft.htail_area)?;
    b.getter("metrics/lh-ft", |m| m.aircraft.htail_arm)?;
    b.getter("metrics/Sv-sqft", |m| m.aircraft.vtail_area)?;
    b.getter("metrics/lv-ft", |m| m.aircraft.vtail_arm)?;
    b.getter("metrics/lh-norm", |m| m.aircraft.lbarh)?;
    b.getter("metrics/lv-norm", |m| m.aircraft.lbarv)?;
    b.getter("metrics/vbarh-norm", |m| m.aircraft.vbarh)?;
    b.getter("metrics/vbarv-norm", |m| m.aircraft.vbarv)?;
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.property(
            &format!("metrics/aero-rp-{name}-in"),
            Kind::Double,
            move |m| m.aircraft.xyz_rp[i],
            move |m, v| m.aircraft.xyz_rp[i] = v,
        )?;
    }
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("metrics/eyepoint-{name}-in"), move |m| {
            m.aircraft.xyz_ep[i]
        })?;
    }
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("metrics/visualrefpoint-{name}-in"), move |m| {
            m.aircraft.xyz_vrp[i]
        })?;
    }
    Ok(())
}
