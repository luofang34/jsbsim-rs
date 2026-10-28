// SPDX-License-Identifier: LGPL-2.1-or-later
// FGGroundReactions: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// FGLGear: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// FGSurface: Copyright (C) 2013 Ansgar Schwarz, Bertrand Coconnier.
// Derived from JSBSim src/models/FGGroundReactions.{h,cpp},
// FGLGear.{h,cpp} (contact geometry and the touching test) and
// FGSurface.{h,cpp}.
//
//! Ground contacts. Only their geometry is ported: a contact that touches
//! the terrain, under the condition `FGLGear::GetBodyForces` uses, stops
//! the simulation with [`Error::Unsupported`] until the gear model is.

use super::base::ModelBase;
use super::inertial::Inertial;
use crate::Error;
use crate::fdm::{Binder, Kind};
use crate::math::{ColumnVector3, Location, Matrix33};
use crate::units::INCHTOFT;
use crate::xml::Element;

/// A `<contact>` (geometry of `FGLGear`).
#[derive(Debug, Clone)]
pub struct Contact {
    /// Contact name.
    pub name: String,
    /// Location, structural frame, in.
    pub location: ColumnVector3,
    /// Whether it retracts.
    pub retractable: bool,
    /// Unit gear position (`GearPos`).
    pub gear_pos: f64,
    /// Sticky switch to the FCS gear position (`useFCSGearPos`).
    pub use_fcs_gear_pos: bool,
}

/// Surface properties (`FGSurface`).
#[derive(Debug, Clone, Copy)]
pub struct Surface {
    /// `ground/solid`.
    pub solid: bool,
    /// `ground/bumpiness`.
    pub bumpiness: f64,
    /// `ground/maximum-force-lbs`.
    pub maximum_force: f64,
    /// `ground/rolling_friction-factor`.
    pub rolling_factor: f64,
    /// `ground/static-friction-factor`.
    pub static_factor: f64,
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            solid: true,
            bumpiness: 0.0,
            maximum_force: f64::MAX,
            rolling_factor: 1.0,
            static_factor: 1.0,
        }
    }
}

/// What the touching test reads.
#[derive(Debug, Clone)]
pub struct Inputs {
    /// Vehicle location.
    pub location: Location,
    /// Body to local.
    pub tb2l: Matrix33,
    /// CG, structural frame, in.
    pub xyz_cg: ColumnVector3,
    /// FCS gear position.
    pub fcs_gear_pos: f64,
}

/// `FGGroundReactions`.
#[derive(Debug, Clone, Default)]
pub struct GroundReactions {
    /// Scheduling.
    pub base: ModelBase,
    /// Contacts in load order.
    pub contacts: Vec<Contact>,
    /// Surface.
    pub surface: Surface,
    /// Steering command.
    pub ds_cmd: f64,
    /// Force, body frame (zero: no contact is ever loaded).
    pub forces: ColumnVector3,
    /// Moment (zero).
    pub moments: ColumnVector3,
}

/// `FGLGear::Ts2b`: structural to body, inches to feet.
fn ts2b() -> Matrix33 {
    Matrix33::new(-INCHTOFT, 0., 0., 0., INCHTOFT, 0., 0., 0., -INCHTOFT)
}

impl GroundReactions {
    /// `Load`: the contacts, after the executive's `Upload`.
    pub fn load(&mut self, document: &Element) -> Result<(), Error> {
        for el in document.find_elements("contact") {
            let name = el.attribute("name").to_owned();
            let Some(location) = el.find_element("location") else {
                return Err(Error::Model {
                    message: format!("{}: No location given for contact {name}", el.location()),
                });
            };
            let retractable = match el.find_element("retractable") {
                Some(_) => el.find_element_value_as_number("retractable")? as u32 > 0,
                None => false,
            };
            self.contacts.push(Contact {
                name,
                location: location.find_element_triplet_convert_to("IN")?,
                retractable,
                gear_pos: 1.0,
                use_fcs_gear_pos: false,
            });
        }
        Ok(())
    }

    /// The touching test of `FGLGear::GetBodyForces`, for every contact in
    /// order: the gear is down and the terrain height under the contact is
    /// negative.
    pub fn check_contacts(
        &mut self,
        input: &Inputs,
        inertial: &Inertial,
        trim_status: bool,
        sim_time: f64,
    ) -> Result<(), Error> {
        for contact in &mut self.contacts {
            let body = ts2b() * (contact.location - input.xyz_cg);
            let local = input.tb2l * body;
            let gear_loc = input.location.local_to_location(local);
            let mut height = inertial.contact(&gear_loc).agl;
            let gear_pos = if contact.retractable {
                if contact.use_fcs_gear_pos || input.fcs_gear_pos != 1.0 {
                    contact.use_fcs_gear_pos = true;
                    input.fcs_gear_pos
                } else {
                    contact.gear_pos
                }
            } else {
                1.0
            };
            if gear_pos > 0.99 {
                if !trim_status {
                    height -= self.surface.bump_height();
                }
                if height < 0.0 {
                    return Err(Error::Unsupported {
                        construct: format!(
                            "ground contact \"{}\" touching the terrain at t={sim_time} s (FGLGear forces)",
                            contact.name
                        ),
                        path: "ground_reactions".to_owned(),
                    });
                }
            }
        }
        Ok(())
    }
}

impl Surface {
    /// `GetBumpHeight` for a flat surface; the executive refuses a
    /// bumpiness of 0.001 or more before the test runs.
    fn bump_height(&self) -> f64 {
        0.0
    }
}

/// `FGGroundReactions::bind` with `FGSurface::bind`.
pub fn bind(b: &mut Binder<'_>) -> Result<(), Error> {
    let d = Kind::Double;
    b.property(
        "ground/solid",
        Kind::Bool,
        |m| f64::from(u8::from(m.ground_reactions.surface.solid)),
        |m, v| {
            m.ground_reactions.surface.solid = v != 0.0;
        },
    )?;
    b.property(
        "ground/bumpiness",
        d,
        |m| m.ground_reactions.surface.bumpiness,
        |m, v| {
            m.ground_reactions.surface.bumpiness = v;
        },
    )?;
    b.property(
        "ground/maximum-force-lbs",
        d,
        |m| m.ground_reactions.surface.maximum_force,
        |m, v| {
            m.ground_reactions.surface.maximum_force = v;
        },
    )?;
    b.property(
        "ground/rolling_friction-factor",
        d,
        |m| m.ground_reactions.surface.rolling_factor,
        |m, v| {
            m.ground_reactions.surface.rolling_factor = v;
        },
    )?;
    b.property(
        "ground/static-friction-factor",
        d,
        |m| m.ground_reactions.surface.static_factor,
        |m, v| {
            m.ground_reactions.surface.static_factor = v;
        },
    )?;
    b.typed_getter("gear/num-units", Kind::Int, |m| {
        m.ground_reactions.contacts.len() as f64
    })?;
    b.typed_getter("gear/wow", Kind::Bool, |_| 0.0)?;
    b.property(
        "fcs/steer-cmd-norm",
        d,
        |m| m.ground_reactions.ds_cmd,
        |m, v| m.ground_reactions.ds_cmd = v,
    )?;
    Ok(())
}
