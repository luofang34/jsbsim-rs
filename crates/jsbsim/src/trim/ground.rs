// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGTrim.cpp (trimOnGround,
// calcRotation).
//
//! The geometric part of the ground trim: the vehicle lowered onto its
//! lowest contact, then rotated about it, and about the line to the next
//! contact it meets, until three contacts touch.

use super::Trim;
use crate::Error;
use crate::fdm::Fdm;
use crate::math::{ColumnVector3, E_PHI, E_PSI, E_THT, Quaternion, dot_product};
use crate::models::ground_reactions::ContactType;

/// `ContactPoints`: a contact in the body frame and the terrain normal
/// under it.
#[derive(Debug, Clone, Copy)]
struct ContactPoint {
    location: ColumnVector3,
    normal: ColumnVector3,
}

/// `calcRotation`'s result: the smallest angle and the contact it brings
/// down.
struct Rotation {
    angle_min: f64,
    contact_ref: Option<usize>,
}

/// `calcRotation`: the angle about axis `u` through `gm0` that brings each
/// contact onto its terrain plane, and the smallest of them.
fn calc_rotation(contacts: &[ContactPoint], u: ColumnVector3, gm0: ColumnVector3) -> Rotation {
    use std::f64::consts::PI;
    let mut r = Rotation {
        angle_min: 3.0 * PI,
        contact_ref: None,
    };
    for (i, c) in contacts.iter().enumerate() {
        let mut t = u.cross(c.normal);
        let length = t.magnitude();
        if length != 0.0 {
            t *= 1.0 / length;
        } else {
            tracing::error!(
                "Attempt to divide by zero in method FGColumnVector3::operator/=(const double scalar), object {} , {} , {}",
                t.to_array()[0],
                t.to_array()[1],
                t.to_array()[2]
            );
        }
        let v = t.cross(u);
        let mm0 = gm0 - c.location;
        let d0 = dot_product(mm0, u);
        let sqr_radius = dot_product(mm0, mm0) - d0 * d0;
        let dist_plane = d0 * dot_product(u, c.normal) / length;
        let mag = sqr_radius - dist_plane * dist_plane;
        if mag < 0.0 {
            tracing::warn!("FGTrim::calcRotation DistPlane^2 larger than sqrRadius");
        }
        let alpha = mag.max(0.0).sqrt();
        let cp = alpha * t + dist_plane * v;
        let cosine = -dot_product(mm0, cp) / sqr_radius;
        let sine = dot_product(mm0.cross(u), cp) / sqr_radius;
        let mut angle = libm::atan2(sine, cosine);
        if angle < 0.0 {
            angle += 2.0 * PI;
        }
        if angle < r.angle_min {
            r.angle_min = angle;
            r.contact_ref = Some(i);
        }
    }
    r
}

impl Trim {
    /// The contacts that are down (and solid, for bogeys), in the body
    /// frame, with the height of the lowest and its index.
    fn ground_contacts(fdm: &mut Fdm) -> (Vec<ContactPoint>, f64, usize) {
        let m = &mut fdm.models;
        let cg_location = m.propagate.state.location;
        let (tec2b, tb2l) = (m.propagate.frames.tec2b, m.propagate.frames.tb2l);
        let gr = &mut m.ground_reactions;
        let (solid, fcs_gear_pos) = (gr.surface.solid, gr.input.fcs_gear_pos);
        let mut contacts = Vec::new();
        let (mut hmin, mut contact_ref) = (1E+10, 0usize);
        for gear in &mut gr.contacts {
            let down = !gear.retractable || gear.gear_unit_pos(fcs_gear_pos) > 0.99;
            if !down {
                continue;
            }
            let location = gear.body_location(&gr.input);
            let gear_loc = cg_location.local_to_location(tb2l * location);
            let contact = m.inertial.contact(&gear_loc);
            if gear.kind == ContactType::Bogey && !solid {
                continue;
            }
            contacts.push(ContactPoint {
                location,
                normal: tec2b * contact.normal,
            });
            if contact.agl < hmin {
                hmin = contact.agl;
                contact_ref = contacts.len() - 1;
            }
        }
        (contacts, hmin, contact_ref)
    }

    /// `trimOnGround`: lowers the initial conditions onto the lowest
    /// contact, then sets the Euler angles that put two more contacts on
    /// the ground, rotating as the accelerations of a frame there push.
    /// Fewer than three contacts down leave the initial conditions as they
    /// are.
    pub(super) fn trim_on_ground(&mut self, fdm: &mut Fdm) -> Result<(), Error> {
        let (mut contacts, hmin, contact_ref) = Self::ground_contacts(fdm);
        if contacts.len() < 3 {
            return Ok(());
        }
        let contact0 = contacts.remove(contact_ref).location;
        let m = &fdm.models;
        let env = crate::initial_condition::Environment::new(&m.atmosphere, &m.inertial);
        let asl = self.fgic.altitude_asl_ft();
        self.fgic.set_altitude_asl_ft(env, asl - hmin);
        fdm.initialize_from(self.fgic.initial_state())?;
        fdm.run()?;

        let m = &fdm.models;
        let force = m.mass_balance.mass * m.accelerations.uvwdot;
        let mut moment = m.mass_balance.j * m.accelerations.pqrdot + force.cross(contact0);
        let rotation_axis = *moment.normalize();

        let r = calc_rotation(&contacts, rotation_axis, contact0);
        let q0 = Quaternion::from_angle_axis(r.angle_min, rotation_axis);
        let rot = q0.t_inv();
        for c in &mut contacts {
            c.location = contact0 + rot * (c.location - contact0);
        }
        let contact1 = contacts.remove(r.contact_ref.unwrap_or(0)).location;
        let mut rotation_axis = contact1 - contact0;
        if dot_product(rotation_axis, moment) < 0.0 {
            rotation_axis = contact0 - contact1;
        }
        rotation_axis.normalize();

        let r = calc_rotation(&contacts, rotation_axis, contact0);
        let q1 = Quaternion::from_angle_axis(r.angle_min, rotation_axis);
        let mut q = &(self.fgic.orientation() * &q0) * &q1;
        q.cache_derived();
        let euler = q.euler();
        self.fgic.set_euler_angle_rad(E_PHI, euler[E_PHI]);
        self.fgic.set_euler_angle_rad(E_THT, euler[E_THT]);
        self.fgic.set_euler_angle_rad(E_PSI, euler[E_PSI]);
        Ok(())
    }
}
