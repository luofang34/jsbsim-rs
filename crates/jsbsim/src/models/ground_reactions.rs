// SPDX-License-Identifier: LGPL-2.1-or-later
// FGGroundReactions: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// FGLGear: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// FGSurface: Copyright (C) 2013 Ansgar Schwarz, Bertrand Coconnier.
// Derived from JSBSim src/models/FGGroundReactions.{h,cpp},
// FGLGear.{h,cpp} and FGSurface.{h,cpp}.
//
//! Ground contacts: what `FGLGear` loads and binds, the forces each
//! computes every frame, and the friction constraints it hands the
//! accelerations solver.

mod contact;
pub(crate) mod contact_bind;
mod force;
mod frame;
mod friction;
mod reporting;
#[cfg(test)]
mod tests;

pub use contact::{
    BrakeGroup, Contact, ContactState, ContactType, DampType, Friction, SteerType, Steering, Strut,
};
pub use contact_bind::indexed_name;
pub use force::{ContactForce, FrictionType, Multipliers, Reporting};
pub use frame::{Compressed, FrameContext};

use super::accelerations::LagrangeMultiplier;
use super::base::ModelBase;
use crate::Error;
use crate::fdm::{Binder, Kind, LoadContext};
use crate::math::{ColumnVector3, Location, Matrix33};
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
    /// `pos`: where bumps are evaluated, which JSBSim itself never moves.
    pub pos: [f64; 3],
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            solid: true,
            bumpiness: 0.0,
            maximum_force: f64::MAX,
            rolling_factor: 1.0,
            static_factor: 1.0,
            pos: [0.0; 3],
        }
    }
}

/// What the executive loads before each run (`FGLGear::Inputs`).
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// Ground speed, ft/s.
    pub vground: f64,
    /// Calibrated airspeed, kt.
    pub vcalibrated_kts: f64,
    /// Ambient temperature, R.
    pub temperature: f64,
    /// Height of the CG above the ground, ft.
    pub distance_agl: f64,
    /// Altitude above sea level, ft.
    pub distance_asl: f64,
    /// `dt` times the model rate, s.
    pub total_delta_t: f64,
    /// `TakeoffThrottle`: whether the first throttle is past 0.9.
    pub takeoff_throttle: bool,
    /// `WOW` of the vehicle, as the previous frame left it.
    pub wow: bool,
    /// Body to local.
    pub tb2l: Matrix33,
    /// ECEF to local.
    pub tec2l: Matrix33,
    /// ECEF to body.
    pub tec2b: Matrix33,
    /// Body rates relative to ECEF, rad/s.
    pub pqr: ColumnVector3,
    /// Body velocity relative to ECEF, ft/s.
    pub uvw: ColumnVector3,
    /// CG, structural frame, in.
    pub xyz_cg: ColumnVector3,
    /// Vehicle location.
    pub location: Location,
    /// FCS brake positions by brake group (`BrakePos`).
    pub brake_pos: [f64; 6],
    /// FCS gear position.
    pub fcs_gear_pos: f64,
    /// Empty weight, lbs.
    pub empty_weight: f64,
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
    /// Force, body frame, lbs.
    pub forces: ColumnVector3,
    /// Moment about the CG, lbs ft.
    pub moments: ColumnVector3,
    /// `multipliers`: the friction constraints registered this frame, as
    /// the contact (by index) and constraint that own each.
    pub multipliers: Vec<(usize, FrictionType)>,
}

impl GroundReactions {
    /// `InitModel`.
    pub fn init_model(&mut self) {
        self.base.init();
        self.forces = ColumnVector3::zero();
        self.moments = ColumnVector3::zero();
        self.ds_cmd = 0.0;
        self.multipliers.clear();
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

    /// The registered friction constraints, copied in order into `out`
    /// for the solver (`GetMultipliersList`).
    pub fn load_multipliers(&self, out: &mut Vec<LagrangeMultiplier>) {
        out.clear();
        for &(c, kind) in &self.multipliers {
            if let Some(contact) = self.contacts.get(c) {
                out.push(*contact.force.multipliers.get(kind));
            }
        }
    }

    /// The solver's values back into the constraints `load_multipliers`
    /// copied them from.
    pub fn store_multipliers(&mut self, solved: &[LagrangeMultiplier]) {
        for (&(c, kind), m) in self.multipliers.iter().zip(solved) {
            if let Some(contact) = self.contacts.get_mut(c) {
                contact.force.multipliers.get_mut(kind).value = m.value;
            }
        }
    }
}

impl Surface {
    /// `GetBumpHeight`.
    #[must_use]
    pub fn bump_height(&self) -> f64 {
        if self.bumpiness < 0.001 {
            return 0.0;
        }
        let mut x = self.pos[0] * 0.1;
        let mut y = self.pos[1] * 0.1;
        x -= libm::floor(x);
        y -= libm::floor(y);
        x *= 2.0 * std::f64::consts::PI;
        y *= 2.0 * std::f64::consts::PI;
        const MAX_GROUND_BUMP_AMPLITUDE: f64 = 0.4;
        let sin = libm::sin;
        let mut h = sin(x) + sin(7.0 * x) + sin(8.0 * x) + sin(13.0 * x);
        h += sin(2.0 * y) + sin(5.0 * y) + sin(9.0 * y * x) + sin(17.0 * y);
        h * (1.0 / 8.) * self.bumpiness * MAX_GROUND_BUMP_AMPLITUDE
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

crate::snapshot::visit_fields! {
    Surface { solid, bumpiness, maximum_force, rolling_factor, static_factor, pos }
}
crate::snapshot::visit_fields! {
    Inputs {
        vground, vcalibrated_kts, temperature, distance_agl, distance_asl, total_delta_t,
        takeoff_throttle, wow, tb2l, tec2l, tec2b, pqr, uvw, xyz_cg, location, brake_pos,
        fcs_gear_pos, empty_weight,
    }
}

/// The friction constraints registered by the last frame that ran the
/// ground reactions, which the next accelerations solve reads.
impl crate::snapshot::Visit for GroundReactions {
    fn visit<V: crate::snapshot::Visitor>(
        &mut self,
        v: &mut V,
    ) -> Result<(), crate::snapshot::SnapshotError> {
        let Self {
            base,
            input,
            contacts,
            surface,
            ds_cmd,
            forces,
            moments,
            multipliers,
        } = self;
        base.visit(v)?;
        input.visit(v)?;
        contacts.visit(v)?;
        surface.visit(v)?;
        ds_cmd.visit(v)?;
        forces.visit(v)?;
        moments.visit(v)?;
        const FRESH: (usize, FrictionType) = (0, FrictionType::Roll);
        let n = v.len(multipliers.len(), "friction constraints")?;
        if v.mutating() {
            multipliers.resize(n, FRESH);
        }
        for (contact, kind) in multipliers.iter_mut().take(n) {
            contact.visit(v)?;
            kind.visit(v)?;
        }
        // A check walks the constraints a restore would append.
        if v.decoding() && n > multipliers.len() {
            let (mut contact, mut kind) = FRESH;
            for _ in multipliers.len()..n {
                contact.visit(v)?;
                kind.visit(v)?;
            }
        }
        Ok(())
    }
}
