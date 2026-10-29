// SPDX-License-Identifier: LGPL-2.1-or-later
// FGLGear: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/FGLGear.cpp (ComputeSlipAngle,
// ComputeBrakeForceCoefficient, ComputeSideForceCoefficient,
// ComputeJacobian).
//
//! A compressed contact's friction: the wheel's coefficients and the
//! constraints the accelerations solver resolves into friction forces.

use super::Inputs;
use super::contact::{BrakeGroup, Contact, ContactType};
use super::force::FrictionType;
use crate::Error;
use crate::functions::FunctionSet;
use crate::math::{ColumnVector3, E_X, E_Y, E_Z};
use crate::units::{RADTODEG, constrain};

impl Contact {
    /// `ComputeSlipAngle`: kept while the wheel is all but still.
    pub(super) fn compute_slip_angle(&mut self) {
        let v = &self.force.ground_wheel_vel;
        if v.magnitude2(E_X, E_Y) > 1E-3 {
            self.state.wheel_slip = -libm::atan2(v[E_Y], v[E_X].abs()) * RADTODEG;
        }
    }

    /// `ComputeBrakeForceCoefficient`: rolling friction plus the braked
    /// share of static friction. A brake group JSBSim leaves undefined
    /// (which strict loading refuses) brakes nothing here.
    pub(super) fn compute_brake_force_coefficient(&mut self, input: &Inputs) {
        let (f, surface) = (&self.friction, &self.surface);
        let mut coeff = surface.rolling_factor * f.rolling_coeff;
        let slot = match self.brake_group {
            Some(BrakeGroup::Left) => Some(1),
            Some(BrakeGroup::Right) => Some(2),
            Some(BrakeGroup::Center) => Some(3),
            Some(BrakeGroup::None) | None => None,
        };
        if let Some(brake) = slot.and_then(|i| input.brake_pos.get(i)) {
            coeff += brake * surface.static_factor * (f.static_coeff - f.rolling_coeff);
        }
        self.force.brake_coeff = coeff;
    }

    /// `ComputeSideForceCoefficient`: the cornering table, else Pacejka's
    /// Magic Formula.
    pub(super) fn compute_side_force_coefficient(
        &mut self,
        functions: &FunctionSet,
    ) -> Result<(), Error> {
        let f = &self.friction;
        let slip = self.state.wheel_slip;
        let mut coeff = if let Some(table) = f.cornering {
            functions.table(table)?.value_1d(slip)
        } else {
            let stiff_slip = f.stiffness * slip;
            let curve = stiff_slip - f.curvature * (stiff_slip - libm::atan(stiff_slip));
            f.peak * libm::sin(f.shape * libm::atan(curve))
        };
        coeff *= self.surface.static_factor;
        self.state.side_coeff = coeff;
        Ok(())
    }

    /// `ComputeJacobian`: the friction constraints at `contact_vec`,
    /// registered in `registry` under the contact's `index`. Each keeps the
    /// value the solver left it, clamped to its new bounds.
    pub(super) fn compute_jacobian(
        &mut self,
        contact_vec: ColumnVector3,
        registry: &mut Vec<(usize, FrictionType)>,
        index: usize,
    ) {
        let f = &mut self.force;
        let normal = f.normal_force[E_Z];
        let static_factor = self.surface.static_factor;
        if self.kind == ContactType::Structure && f.ground_wheel_vel.magnitude2(E_X, E_Y) > 1E-3 {
            let mut direction = f.ground_wheel_vel;
            f.static_friction = false;
            direction[E_Z] = 0.;
            direction.normalize();
            let m = &mut f.multipliers.dynamic;
            m.force_jacobian = f.t_ground * direction;
            m.max = 0.;
            m.min = -(static_factor * self.friction.dynamic_coeff * normal).abs();
            m.lever_arm = contact_vec;
            m.value = constrain(m.min, m.value, m.max);
            registry.push((index, FrictionType::Dynamic));
            return;
        }
        f.static_friction = true;
        let m = &mut f.multipliers;
        m.roll.force_jacobian = f.t_ground * ColumnVector3::new(1., 0., 0.);
        m.side.force_jacobian = f.t_ground * ColumnVector3::new(0., 1., 0.);
        m.roll.lever_arm = contact_vec;
        m.side.lever_arm = contact_vec;
        match self.kind {
            ContactType::Bogey => {
                m.roll.max = (f.brake_coeff * normal).abs();
                m.side.max = (self.state.side_coeff * normal).abs();
            }
            ContactType::Structure => {
                m.roll.max = (static_factor * self.friction.static_coeff * normal).abs();
                m.side.max = m.roll.max;
            }
        }
        m.roll.min = -m.roll.max;
        m.side.min = -m.side.max;
        m.roll.value = constrain(m.roll.min, m.roll.value, m.roll.max);
        m.side.value = constrain(m.side.min, m.side.value, m.side.max);
        registry.push((index, FrictionType::Roll));
        registry.push((index, FrictionType::Side));
    }
}
