// SPDX-License-Identifier: LGPL-2.1-or-later
// FGExternalReactions, FGExternalForce: Copyright (C) 2007 Jon S. Berndt.
// FGForce: Copyright (C) 1999 Tony Peden.
// Derived from JSBSim src/models/FGExternalReactions.{h,cpp},
// FGExternalForce.{h,cpp} and src/models/propulsion/FGForce.{h,cpp}.
//
//! Arbitrary forces and moments defined in the aircraft file, in the body,
//! local, wind or inertial frame.

use super::base::ModelBase;
use crate::Error;
use crate::fdm::{Kind, LoadContext};
use crate::functions::FnId;
use crate::math::{ColumnVector3, E_L, E_M, E_N, E_X, E_Y, E_Z, Matrix33};
use crate::properties::NodeId;
use crate::xml::Element;

/// `FGForce::TransformType` of an external force.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// Body axes (`tNone`).
    Body,
    /// Local NED axes.
    Local,
    /// Wind axes.
    Wind,
    /// ECI axes.
    Inertial,
}

/// Where the magnitude comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Magnitude {
    /// A `<function>`.
    Function(FnId),
    /// The `.../magnitude` property.
    Property(NodeId),
}

/// One `<force>` or `<moment>` (`FGExternalForce`).
#[derive(Debug, Clone)]
pub struct ExternalForce {
    /// Name.
    pub name: String,
    /// True for a `<moment>`.
    pub is_moment: bool,
    /// Frame of the direction.
    pub frame: Frame,
    /// Magnitude source.
    pub magnitude: Magnitude,
    /// Direction nodes (`x y z` or `l m n`).
    pub direction: [NodeId; 3],
    /// Location, structural frame, in (`vXYZn`).
    pub location: ColumnVector3,
    /// Point the force acts at (`vActingXYZn`).
    pub acting_location: ColumnVector3,
    /// Body-frame force of the last frame (`vFb`).
    pub body_force: ColumnVector3,
    /// Moment about the CG of the last frame (`vM`).
    pub moment: ColumnVector3,
}

/// What a force needs from the rest of the simulation.
#[derive(Debug, Clone, Copy)]
pub struct Transforms {
    /// Wind to body.
    pub tw2b: Matrix33,
    /// Local to body.
    pub tl2b: Matrix33,
    /// ECI to body.
    pub ti2b: Matrix33,
}

impl ExternalForce {
    fn transform(&self, t: &Transforms) -> Matrix33 {
        match self.frame {
            Frame::Body => Matrix33::new(1., 0., 0., 0., 1., 0., 0., 0., 1.),
            Frame::Local => t.tl2b,
            Frame::Wind => t.tw2b,
            Frame::Inertial => t.ti2b,
        }
    }

    /// `GetBodyForces`: from the magnitude and direction just read;
    /// `to_body` is `MassBalance::StructuralToBody`.
    pub fn update(
        &mut self,
        magnitude: f64,
        direction: ColumnVector3,
        t: &Transforms,
        to_body: impl Fn(ColumnVector3) -> ColumnVector3,
    ) {
        let (mut fn_, mut mn) = (ColumnVector3::zero(), ColumnVector3::zero());
        if self.is_moment {
            mn = self.transform(t) * (magnitude * direction);
        } else {
            fn_ = magnitude * direction;
        }
        self.body_force = self.transform(t) * fn_;
        let dxyz = to_body(self.acting_location);
        self.moment = mn + dxyz.cross(self.body_force);
    }
}

/// `FGExternalReactions`.
#[derive(Debug, Clone, Default)]
pub struct ExternalReactions {
    /// Scheduling.
    pub base: ModelBase,
    /// Forces and moments in load order.
    pub forces: Vec<ExternalForce>,
    /// Total force, body frame.
    pub total_forces: ColumnVector3,
    /// Total moment about the CG.
    pub total_moments: ColumnVector3,
}

fn parse_frame(el: &Element, name: &str) -> Frame {
    match el.attribute("frame") {
        "BODY" => Frame::Body,
        "LOCAL" => Frame::Local,
        "WIND" => Frame::Wind,
        "INERTIAL" => Frame::Inertial,
        other => {
            tracing::warn!(
                "{}: Invalid frame type \"{other}\" for external force \"{name}\". Frame set to Body",
                el.location()
            );
            Frame::Body
        }
    }
}

impl ExternalReactions {
    /// A force's `<location>` and its tied `location-*-in` properties.
    fn load_location(
        &self,
        el: &Element,
        force: &mut ExternalForce,
        base_name: &str,
        ctx: &mut LoadContext<'_>,
    ) -> Result<(), Error> {
        let index = self.forces.len();
        match el.find_element("location") {
            None => tracing::warn!(
                "{}: No location element specified in force object.",
                el.location()
            ),
            Some(l) => {
                force.location = l.find_element_triplet_convert_to("IN")?;
                force.acting_location = force.location;
            }
        }
        for (axis, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
            ctx.binder.property(
                &format!("{base_name}/location-{axis}-in"),
                Kind::Double,
                // Forces are never removed, so `index` always names this one.
                move |m| {
                    m.external_reactions
                        .forces
                        .get(index)
                        .map_or(0.0, |f| f.location[i])
                },
                move |m, v| {
                    if let Some(f) = m.external_reactions.forces.get_mut(index) {
                        f.location[i] = v;
                        f.acting_location[i] = v;
                    }
                },
            )?;
        }
        Ok(())
    }

    /// `InitModel`.
    pub fn init_model(&mut self) {
        self.base.init();
        self.total_forces = ColumnVector3::zero();
        self.total_moments = ColumnVector3::zero();
    }

    /// `Load`: every `<force>`, then every `<moment>`.
    pub fn load(&mut self, el: &Element, ctx: &mut LoadContext<'_>) -> Result<(), Error> {
        for force in el.find_elements("force") {
            self.load_one(force, false, ctx)?;
        }
        for moment in el.find_elements("moment") {
            self.load_one(moment, true, ctx)?;
        }
        Ok(())
    }

    fn load_one(
        &mut self,
        el: &Element,
        is_moment: bool,
        ctx: &mut LoadContext<'_>,
    ) -> Result<(), Error> {
        let name = el.attribute("name").to_owned();
        let base_name = format!("external_reactions/{name}");
        let axes = if is_moment {
            ["l", "m", "n"]
        } else {
            ["x", "y", "z"]
        };
        let tree = ctx.tree();
        let mut direction = [tree.root(); 3];
        for (slot, axis) in direction.iter_mut().zip(axes) {
            *slot = tree.get_or_create(tree.root(), &format!("{base_name}/{axis}"))?;
        }
        let frame = parse_frame(el, &name);
        match el.find_element("direction") {
            None => tracing::warn!(
                "{}: No direction element specified in force object. Default is (0,0,0).",
                el.location()
            ),
            Some(d) => {
                let mut v = d.find_element_triplet_convert_to("IN")?;
                v.normalize();
                // FGPropertyVector3::operator= writes y, then x, then z.
                let [dx, dy, dz] = direction;
                for (node, axis) in [(dy, E_Y), (dx, E_X), (dz, E_Z)] {
                    ctx.tree().set_double(node, v[axis]);
                }
            }
        }
        let magnitude = match el.find_element("function") {
            Some(f) => Magnitude::Function(ctx.functions.load_function(ctx.binder.tree, f, "")?),
            None => {
                let suffix = if is_moment {
                    "magnitude-lbsft"
                } else {
                    "magnitude"
                };
                let tree = ctx.tree();
                Magnitude::Property(
                    tree.get_or_create(tree.root(), &format!("{base_name}/{suffix}"))?,
                )
            }
        };
        let mut force = ExternalForce {
            name,
            is_moment,
            frame,
            magnitude,
            direction,
            location: ColumnVector3::zero(),
            acting_location: ColumnVector3::zero(),
            body_force: ColumnVector3::zero(),
            moment: ColumnVector3::zero(),
        };
        if !is_moment {
            self.load_location(el, &mut force, &base_name, ctx)?;
        }
        self.forces.push(force);
        Ok(())
    }
}

/// `FGExternalReactions::bind`.
pub fn bind(b: &mut crate::fdm::Binder<'_>) -> Result<(), Error> {
    for (name, i) in [("l", E_L), ("m", E_M), ("n", E_N)] {
        b.getter(&format!("moments/{name}-external-lbsft"), move |m| {
            m.external_reactions.total_moments[i]
        })?;
    }
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("forces/fb{name}-external-lbs"), move |m| {
            m.external_reactions.total_forces[i]
        })?;
    }
    Ok(())
}
