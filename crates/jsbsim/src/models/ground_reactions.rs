// SPDX-License-Identifier: LGPL-2.1-or-later
// FGGroundReactions: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// FGLGear: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// FGSurface: Copyright (C) 2013 Ansgar Schwarz, Bertrand Coconnier.
// Derived from JSBSim src/models/FGGroundReactions.{h,cpp},
// FGLGear.{h,cpp} (GetBodyForces up to the contact test) and
// FGSurface.{h,cpp}.
//
//! Ground contacts. Everything `FGLGear` loads and binds is ported, and
//! each frame runs `GetBodyForces` as far as a contact that is not
//! compressed goes; a contact that touches the terrain stops the
//! simulation with [`Error::Touchdown`] until the gear forces are.

mod contact;
mod contact_bind;
#[cfg(test)]
mod tests;

pub use contact::{
    BrakeGroup, Contact, ContactState, ContactType, DampType, Friction, SteerType, Steering, Strut,
};
pub use contact_bind::indexed_name;

use super::base::ModelBase;
use super::inertial::Inertial;
use crate::Error;
use crate::fdm::{Binder, Kind, LoadContext};
use crate::math::{ColumnVector3, E_X, Location, Matrix33};
use crate::units::INCHTOFT;
use crate::xml::Element;

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

/// What the executive loads before each run (`FGGroundReactions::in`,
/// the fields the airborne part of `FGLGear::GetBodyForces` and the
/// constructor read).
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// Vehicle location.
    pub location: Location,
    /// Body to local.
    pub tb2l: Matrix33,
    /// CG, structural frame, in.
    pub xyz_cg: ColumnVector3,
    /// FCS gear position.
    pub fcs_gear_pos: f64,
    /// Empty weight, lbs.
    pub empty_weight: f64,
    /// `dt` times the model rate, s.
    pub total_delta_t: f64,
}

/// `FGGroundReactions`.
#[derive(Debug, Clone, Default)]
pub struct GroundReactions {
    /// Scheduling.
    pub base: ModelBase,
    /// Inputs.
    pub input: Inputs,
    /// Contacts in load order.
    pub contacts: Vec<Contact>,
    /// Surface.
    pub surface: Surface,
    /// Steering command.
    pub ds_cmd: f64,
    /// Force, body frame (zero: no contact force is computed).
    pub forces: ColumnVector3,
    /// Moment (zero).
    pub moments: ColumnVector3,
}

/// `FGLGear::Ts2b`: structural to body, inches to feet.
fn ts2b() -> Matrix33 {
    Matrix33::new(-INCHTOFT, 0., 0., 0., INCHTOFT, 0., 0., 0., -INCHTOFT)
}

impl GroundReactions {
    /// `InitModel`.
    pub fn init_model(&mut self) {
        self.base.init();
        self.forces = ColumnVector3::zero();
        self.moments = ColumnVector3::zero();
        self.ds_cmd = 0.0;
        for contact in &mut self.contacts {
            contact.reset_to_ic();
        }
    }

    /// `Load`: the contacts, after the executive's `Upload`, each bound as
    /// it is constructed.
    pub fn load(&mut self, document: &Element, ctx: &mut LoadContext<'_>) -> Result<(), Error> {
        for el in document.find_elements("contact") {
            let number = self.contacts.len();
            let contact = Contact::load(el, number, self.input.empty_weight, ctx)?;
            contact_bind::bind(&mut ctx.binder, &contact)?;
            self.contacts.push(contact);
        }
        Ok(())
    }

    /// `GetWOW`: whether any BOGEY has weight on it.
    #[must_use]
    pub fn wow(&self) -> bool {
        self.contacts
            .iter()
            .any(|c| c.kind == ContactType::Bogey && c.state.wow)
    }

    /// `SetDsCmd`.
    pub fn set_ds_cmd(&mut self, cmd: f64) {
        self.ds_cmd = cmd;
        for contact in &mut self.contacts {
            contact.set_steer_cmd(cmd);
        }
    }

    /// `FGLGear::GetBodyForces` for every contact in order, up to the
    /// point where a contact first compresses against the terrain, which is
    /// refused with [`Error::Touchdown`].
    pub fn check_contacts(
        &mut self,
        inertial: &Inertial,
        trim_status: bool,
        sim_time: f64,
    ) -> Result<(), Error> {
        let bump = self.surface.bump_height();
        for contact in &mut self.contacts {
            if contact.airborne_frame(
                &self.input,
                inertial,
                &self.surface,
                bump,
                trim_status,
                sim_time,
            ) {
                return Err(Error::Touchdown {
                    contact: contact.name.clone(),
                    time: sim_time,
                });
            }
        }
        Ok(())
    }
}

/// `std::max(a, b)`, which is `a < b ? b : a`: unlike [`f64::max`], it
/// keeps `a` when either is NaN and when both are zeros.
fn cpp_max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

impl Contact {
    /// `GetBodyForces` while the contact is not compressed. Returns true,
    /// having changed nothing past the height test, where JSBSim would
    /// start computing the gear forces.
    fn airborne_frame(
        &mut self,
        input: &Inputs,
        inertial: &Inertial,
        surface: &Surface,
        bump: f64,
        trim_status: bool,
        sim_time: f64,
    ) -> bool {
        let mut gear_pos = 1.0;
        let body = ts2b() * (self.location - input.xyz_cg);
        self.state.local_gear = input.tb2l * body;
        let gear_loc = input.location.local_to_location(self.state.local_gear);
        let mut height = inertial.contact(&gear_loc).agl;
        self.state.agl = cpp_max(height, 0.0);
        if self.retractable {
            gear_pos = self.gear_unit_pos(input.fcs_gear_pos);
        }
        if gear_pos > 0.99 {
            if !trim_status {
                height -= bump;
            }
            self.surface.static_factor = surface.static_factor;
            self.surface.rolling_factor = surface.rolling_factor;
            self.surface.maximum_force = surface.maximum_force;
            self.surface.bumpiness = surface.bumpiness;
            self.surface.solid = surface.solid;
            if height < 0.0 {
                return true;
            }
            let s = &mut self.state;
            s.wow = false;
            s.compress_length = 0.0;
            s.compress_speed = 0.0;
            s.wheel_slip = 0.0;
            s.strut_force = 0.0;
            self.steering.angle *= cpp_max(gear_pos - 0.8, 0.0) / 0.2;
        }
        let s = &mut self.state;
        if !s.wow {
            s.wheel_vel[E_X] -= 13.0 * input.total_delta_t;
            if s.wheel_vel[E_X] < 0.0 {
                s.wheel_vel[E_X] = 0.0;
            }
        }
        if !trim_status {
            if s.last_wow != s.wow {
                tracing::info!(
                    "GEAR_CONTACT: {sim_time:.6} seconds: {} {}",
                    self.name,
                    u8::from(s.wow)
                );
            }
            s.last_wow = s.wow;
        }
        false
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
    b.typed_getter("gear/wow", Kind::Bool, |m| {
        f64::from(u8::from(m.ground_reactions.wow()))
    })?;
    b.property(
        "fcs/steer-cmd-norm",
        d,
        |m| m.ground_reactions.ds_cmd,
        |m, v| m.ground_reactions.set_ds_cmd(v),
    )?;
    Ok(())
}
