// SPDX-License-Identifier: LGPL-2.1-or-later
// FGLGear: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/FGLGear.{h,cpp} (the constructor,
// ResetToIC and the setters).
//
//! One `<contact>`: what `FGLGear`'s constructor reads, and the state its
//! properties expose.

use super::Surface;
use super::force::{ContactForce, Multipliers, Reporting};
use crate::Error;
use crate::fdm::LoadContext;
use crate::functions::{FnId, TableId};
use crate::math::{ColumnVector3, E_X, E_Y, Matrix33, Quaternion};
use crate::units::{DEGTORAD, RADTODEG};
use crate::xml::Element;

/// `FGLGear::ContactType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactType {
    /// A wheel (`ctBOGEY`): tied under `gear/unit[n]`.
    Bogey,
    /// A structural point (`ctSTRUCTURE`): tied under `contact/unit[n]`.
    Structure,
}

/// `FGLGear::SteerType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteerType {
    /// Steered by the FCS (`stSteer`).
    Steer,
    /// Not steerable (`stFixed`).
    Fixed,
    /// Free to caster (`stCaster`).
    Caster,
}

/// `FGLGear::BrakeGroup` (`NOSE` and `TAIL` map to `Center`, as in JSBSim).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrakeGroup {
    /// `bgNone`.
    None,
    /// `bgLeft`.
    Left,
    /// `bgRight`.
    Right,
    /// `bgCenter`.
    Center,
}

/// `FGLGear::DampType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DampType {
    /// Force proportional to the compression speed.
    Linear,
    /// Force proportional to its square (`type="SQUARE"`).
    Square,
}

/// The strut: spring and damping, or a `<strut_force>` function.
#[derive(Debug, Clone, Copy)]
pub struct Strut {
    /// `kSpring`, lbs/ft.
    pub k_spring: f64,
    /// `bDamp`, lbs/ft/s or lbs/ft²/s².
    pub b_damp: f64,
    /// `bDampRebound`.
    pub b_damp_rebound: f64,
    /// `eDampType`.
    pub damp: DampType,
    /// `eDampTypeRebound`.
    pub damp_rebound: DampType,
    /// `fStrutForce`, which replaces the spring and damping when present.
    pub function: Option<FnId>,
}

/// Friction coefficients and the Pacejka factors.
#[derive(Debug, Clone, Copy)]
pub struct Friction {
    /// `staticFCoeff`.
    pub static_coeff: f64,
    /// `dynamicFCoeff`.
    pub dynamic_coeff: f64,
    /// `rollingFCoeff`.
    pub rolling_coeff: f64,
    /// `Stiffness`.
    pub stiffness: f64,
    /// `Shape`.
    pub shape: f64,
    /// `Peak`: follows the static coefficient.
    pub peak: f64,
    /// `Curvature`.
    pub curvature: f64,
    /// `CORNERING_COEFF` table (`ForceY_Table`).
    pub cornering: Option<TableId>,
}

/// Steering limits and angle.
#[derive(Debug, Clone, Copy)]
pub struct Steering {
    /// `maxSteerAngle`, deg.
    pub max_angle: f64,
    /// `eSteerType`.
    pub kind: SteerType,
    /// `Castered`.
    pub castered: bool,
    /// `SteerAngle`, rad.
    pub angle: f64,
}

/// What `GetBodyForces` updates every frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContactState {
    /// `WOW`.
    pub wow: bool,
    /// `lastWOW`.
    pub last_wow: bool,
    /// `AGL`, ft: the uncompressed contact's height, never negative. 0
    /// until the first frame computes it; C++ leaves it uninitialized
    /// until then (FGLGear.h:369).
    pub agl: f64,
    /// `compressLength`, ft.
    pub compress_length: f64,
    /// `compressSpeed`, ft/s.
    pub compress_speed: f64,
    /// `WheelSlip`, deg.
    pub wheel_slip: f64,
    /// `StrutForce`, lbs.
    pub strut_force: f64,
    /// `FCoeff`, the side friction coefficient. 0 until the gear forces
    /// first compute it; C++ leaves it uninitialized until then
    /// (FGLGear.h:350).
    pub side_coeff: f64,
    /// `vWhlVelVec`, gear frame, ft/s.
    pub wheel_vel: ColumnVector3,
    /// `vLocalGear`: contact location relative to the CG, local frame, ft.
    pub local_gear: ColumnVector3,
}

/// A `<contact>` (`FGLGear`).
#[derive(Debug, Clone)]
pub struct Contact {
    /// Contact name.
    pub name: String,
    /// `GearNumber`: index in load order.
    pub number: usize,
    /// `eContactType`.
    pub kind: ContactType,
    /// Location, structural frame, in (`vXYZn`).
    pub location: ColumnVector3,
    /// Point the force acts at (`vActingXYZn`).
    pub acting_location: ColumnVector3,
    /// Strut.
    pub strut: Strut,
    /// Friction.
    pub friction: Friction,
    /// Steering.
    pub steering: Steering,
    /// `eBrakeGrp`; `None` where JSBSim logs an undefined group and leaves
    /// it unset.
    pub brake_group: Option<BrakeGroup>,
    /// `mTGear`: gear to body orientation.
    pub t_gear: Matrix33,
    /// Whether it retracts.
    pub retractable: bool,
    /// Unit gear position (`GearPos`).
    pub gear_pos: f64,
    /// Sticky switch to the FCS gear position (`useFCSGearPos`).
    pub use_fcs_gear_pos: bool,
    /// Surface properties copied from the ground each frame the gear is
    /// down.
    pub surface: Surface,
    /// Per-frame state.
    pub state: ContactState,
    /// What the contact computes while compressed.
    pub force: ContactForce,
    /// Takeoff and landing reporting.
    pub reporting: Reporting,
}

/// JSBSim logs `message` and goes on; strict loading refuses it.
fn lenient(ctx: &LoadContext<'_>, el: &Element, message: String) -> Result<(), Error> {
    if ctx.options.strict {
        return Err(Error::Model {
            message: format!("{}: {message} (refused in strict mode)", el.location()),
        });
    }
    tracing::error!("{}: {message}", el.location());
    Ok(())
}

impl Contact {
    /// `FGLGear(el, fdmex, number, in)` up to, not including, `bind`.
    /// `empty_weight` is the executive's last `EmptyWeight` input, the
    /// stiffness a STRUCTURE contact defaults to.
    pub fn load(
        el: &Element,
        number: usize,
        empty_weight: f64,
        ctx: &mut LoadContext<'_>,
    ) -> Result<Self, Error> {
        let name = el.attribute("name").to_owned();
        // JSBSim treats an unknown type as STRUCTURE without a word.
        let kind = match el.attribute("type") {
            "BOGEY" => ContactType::Bogey,
            _ => ContactType::Structure,
        };
        let mut contact = Self::defaults(name, number, kind, empty_weight);
        contact.load_strut(el, ctx)?;
        contact.load_friction(el)?;
        contact.load_steering(el)?;
        contact.load_tables(el, ctx)?;
        let Some(location) = el.find_element("location") else {
            return Err(Error::Model {
                message: format!(
                    "{}: No location given for contact {}",
                    el.location(),
                    contact.name
                ),
            });
        };
        contact.location = location.find_element_triplet_convert_to("IN")?;
        if let Some(orientation) = el.find_element("orientation")
            && kind == ContactType::Bogey
        {
            let euler = orientation.find_element_triplet_convert_to("RAD")?;
            contact.t_gear = Quaternion::from_euler_vector(euler).t();
        }
        contact.load_brake_group(el, ctx)?;
        contact.friction.peak = contact.friction.static_coeff;
        Ok(contact)
    }

    fn defaults(name: String, number: usize, kind: ContactType, empty_weight: f64) -> Self {
        let mut strut = Strut {
            k_spring: 0.0,
            b_damp: 0.0,
            b_damp_rebound: 0.0,
            damp: DampType::Linear,
            damp_rebound: DampType::Linear,
            function: None,
        };
        let mut friction = Friction {
            static_coeff: 0.0,
            dynamic_coeff: 0.0,
            rolling_coeff: 0.0,
            stiffness: 0.06,
            shape: 2.8,
            peak: 0.0,
            curvature: 1.03,
            cornering: None,
        };
        if kind == ContactType::Structure {
            strut.k_spring = empty_weight;
            strut.b_damp = strut.k_spring;
            strut.b_damp_rebound = strut.k_spring * 10.0;
            friction.static_coeff = 1.0;
            friction.dynamic_coeff = 1.0;
        }
        Self {
            name,
            number,
            kind,
            location: ColumnVector3::zero(),
            acting_location: ColumnVector3::zero(),
            strut,
            friction,
            steering: Steering {
                max_angle: 0.0,
                kind: SteerType::Steer,
                castered: false,
                angle: 0.0,
            },
            brake_group: None,
            t_gear: Matrix33::new(1., 0., 0., 0., 1., 0., 0., 0., 1.),
            retractable: false,
            gear_pos: 1.0,
            use_fcs_gear_pos: false,
            surface: Surface::default(),
            state: ContactState::default(),
            force: ContactForce::default(),
            reporting: Reporting::default(),
        }
    }

    fn load_strut(&mut self, el: &Element, ctx: &mut LoadContext<'_>) -> Result<(), Error> {
        if let Some(strut_force) = el.find_element("strut_force") {
            let Some(function) = strut_force.find_element("function") else {
                return Err(Error::Model {
                    message: format!(
                        "{}: <strut_force> of contact {} has no <function>",
                        strut_force.location(),
                        self.name
                    ),
                });
            };
            self.strut.function =
                Some(ctx.functions.load_function(ctx.binder.tree, function, "")?);
            return Ok(());
        }
        let s = &mut self.strut;
        if el.find_element("spring_coeff").is_some() {
            s.k_spring = el.find_element_value_as_number_convert_to("spring_coeff", "LBS/FT")?;
        }
        if let Some((damp, b)) = damping(el, "damping_coeff")? {
            (s.damp, s.b_damp) = (damp, b);
        }
        match damping(el, "damping_coeff_rebound")? {
            Some((damp, b)) => (s.damp_rebound, s.b_damp_rebound) = (damp, b),
            None => (s.damp_rebound, s.b_damp_rebound) = (s.damp, s.b_damp),
        }
        Ok(())
    }

    fn load_friction(&mut self, el: &Element) -> Result<(), Error> {
        let f = &mut self.friction;
        for (tag, slot) in [
            ("dynamic_friction", &mut f.dynamic_coeff),
            ("static_friction", &mut f.static_coeff),
            ("rolling_friction", &mut f.rolling_coeff),
        ] {
            if el.find_element(tag).is_some() {
                *slot = el.find_element_value_as_number(tag)?;
            }
        }
        if el.find_element("retractable").is_some() {
            self.retractable = el.find_element_value_as_number("retractable")? as u32 > 0;
        }
        Ok(())
    }

    fn load_steering(&mut self, el: &Element) -> Result<(), Error> {
        let s = &mut self.steering;
        if el.find_element("max_steer").is_some() {
            s.max_angle = el.find_element_value_as_number_convert_to("max_steer", "DEG")?;
        }
        let castered = match el.find_element("castered") {
            Some(c) => Some(c.data_as_number()? != 0.0),
            None => None,
        };
        if (s.max_angle == 360.0 && castered.is_none()) || castered == Some(true) {
            s.kind = SteerType::Caster;
            s.castered = true;
        } else if s.max_angle == 0.0 {
            s.kind = SteerType::Fixed;
        } else {
            s.kind = SteerType::Steer;
        }
        Ok(())
    }

    fn load_tables(&mut self, el: &Element, ctx: &mut LoadContext<'_>) -> Result<(), Error> {
        for table in el.find_elements("table") {
            if table.attribute("name") == "CORNERING_COEFF" {
                let id = ctx.functions.load_table(ctx.binder.tree, table, "")?;
                self.friction.cornering = Some(id);
                break;
            }
            let message = format!("Undefined force table for {} contact point", self.name);
            lenient(ctx, table, message)?;
        }
        Ok(())
    }

    fn load_brake_group(&mut self, el: &Element, ctx: &LoadContext<'_>) -> Result<(), Error> {
        self.brake_group = match el.find_element_value("brake_group") {
            "LEFT" => Some(BrakeGroup::Left),
            "RIGHT" => Some(BrakeGroup::Right),
            "CENTER" | "NOSE" | "TAIL" => Some(BrakeGroup::Center),
            "NONE" | "" => Some(BrakeGroup::None),
            other => {
                let message = format!(
                    "Improper braking group specification in config file: {other} is undefined."
                );
                lenient(ctx, el, message)?;
                None
            }
        };
        Ok(())
    }

    /// `ResetToIC`.
    pub fn reset_to_ic(&mut self) {
        self.gear_pos = 1.0;
        let s = &mut self.state;
        s.wow = false;
        s.last_wow = false;
        s.wheel_vel = ColumnVector3::zero();
        s.compress_length = 0.0;
        s.compress_speed = 0.0;
        s.wheel_slip = 0.0;
        self.steering.angle = 0.0;
        self.reporting.reset_to_ic();
        self.force.multipliers = Multipliers::default();
    }

    /// `GetGearUnitPos`: the FCS gear position once it has left 1.
    pub fn gear_unit_pos(&mut self, fcs_gear_pos: f64) -> f64 {
        if self.use_fcs_gear_pos || fcs_gear_pos != 1.0 {
            self.use_fcs_gear_pos = true;
            return fcs_gear_pos;
        }
        self.gear_pos
    }

    /// `SetstaticFCoeff`.
    pub fn set_static_coeff(&mut self, coeff: f64) {
        self.friction.static_coeff = coeff;
        self.friction.peak = coeff;
    }

    /// `GetSteerAngleDeg`.
    #[must_use]
    pub fn steer_angle_deg(&self) -> f64 {
        RADTODEG * self.steering.angle
    }

    /// `SetSteerAngleDeg`: ignored by fixed and castered contacts.
    pub fn set_steer_angle_deg(&mut self, angle: f64) {
        if self.steering.kind != SteerType::Fixed && !self.steering.castered {
            self.steering.angle = DEGTORAD * angle;
        }
    }

    /// `SetSteerCmd`.
    pub fn set_steer_cmd(&mut self, cmd: f64) {
        self.set_steer_angle_deg(cmd * self.steering.max_angle);
    }

    /// `GetWheelRollVel`.
    #[must_use]
    pub fn wheel_roll_vel(&self) -> f64 {
        let (v, a) = (&self.state.wheel_vel, self.steering.angle);
        v[E_X] * libm::cos(a) + v[E_Y] * libm::sin(a)
    }
}

/// A `<damping_coeff>`-like child: its type and value in the unit the type
/// calls for.
fn damping(el: &Element, tag: &str) -> Result<Option<(DampType, f64)>, Error> {
    let Some(child) = el.find_element(tag) else {
        return Ok(None);
    };
    Ok(Some(if child.attribute("type") == "SQUARE" {
        (
            DampType::Square,
            el.find_element_value_as_number_convert_to(tag, "LBS/FT2/SEC2")?,
        )
    } else {
        (
            DampType::Linear,
            el.find_element_value_as_number_convert_to(tag, "LBS/FT/SEC")?,
        )
    }))
}
