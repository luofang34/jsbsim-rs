// SPDX-License-Identifier: LGPL-2.1-or-later
// FGLGear: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/FGLGear.{h,cpp} (the force, friction and
// reporting members) and src/models/propulsion/FGForce.h.
//
//! What a contact computes while it is compressed against the ground, and
//! the takeoff and landing figures it reports.

use crate::math::{ColumnVector3, Matrix33};
use crate::models::accelerations::LagrangeMultiplier;

/// `FGLGear::FrictionType`: which of a contact's friction constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrictionType {
    /// `ftRoll`: along the rolling direction.
    Roll,
    /// `ftSide`: across it.
    Side,
    /// `ftDynamic`: against the sliding velocity.
    Dynamic,
}

/// `LMultiplier[3]`: the friction constraints, whose values carry from
/// frame to frame as the solver's starting point.
#[derive(Debug, Clone, Copy, Default)]
pub struct Multipliers {
    /// `LMultiplier[ftRoll]`.
    pub roll: LagrangeMultiplier,
    /// `LMultiplier[ftSide]`.
    pub side: LagrangeMultiplier,
    /// `LMultiplier[ftDynamic]`.
    pub dynamic: LagrangeMultiplier,
}

impl Multipliers {
    /// `LMultiplier[kind]`.
    #[must_use]
    pub fn get(&self, kind: FrictionType) -> &LagrangeMultiplier {
        match kind {
            FrictionType::Roll => &self.roll,
            FrictionType::Side => &self.side,
            FrictionType::Dynamic => &self.dynamic,
        }
    }

    /// `LMultiplier[kind]`, mutably.
    pub fn get_mut(&mut self, kind: FrictionType) -> &mut LagrangeMultiplier {
        match kind {
            FrictionType::Roll => &mut self.roll,
            FrictionType::Side => &mut self.side,
            FrictionType::Dynamic => &mut self.dynamic,
        }
    }
}

/// The force members of `FGLGear` and of its `FGForce` base.
#[derive(Debug, Clone, Copy)]
pub struct ContactForce {
    /// `mT`: ground frame (roll, side, normal columns) to body.
    pub t_ground: Matrix33,
    /// `vGroundNormal`, body frame.
    pub ground_normal: ColumnVector3,
    /// `vGroundWhlVel`: contact velocity, ground frame, ft/s.
    pub ground_wheel_vel: ColumnVector3,
    /// `vFn`: the force in the ground frame, lbs; only its normal
    /// component is set, friction coming from the solver.
    pub normal_force: ColumnVector3,
    /// `vFb`: the force in the body frame, lbs.
    pub body_force: ColumnVector3,
    /// `vM`: its moment about the CG, lbs ft.
    pub moment: ColumnVector3,
    /// `BrakeFCoeff`.
    pub brake_coeff: f64,
    /// `StaticFriction`: whether the roll and side constraints are the
    /// ones registered.
    pub static_friction: bool,
    /// `LMultiplier`.
    pub multipliers: Multipliers,
}

impl Default for ContactForce {
    fn default() -> Self {
        Self {
            t_ground: Matrix33::new(1., 0., 0., 0., 1., 0., 0., 0., 1.),
            ground_normal: ColumnVector3::zero(),
            ground_wheel_vel: ColumnVector3::zero(),
            normal_force: ColumnVector3::zero(),
            body_force: ColumnVector3::zero(),
            moment: ColumnVector3::zero(),
            brake_coeff: 0.0,
            static_friction: false,
            multipliers: Multipliers::default(),
        }
    }
}

/// The takeoff and landing report members of `FGLGear`.
#[derive(Debug, Clone, Copy)]
pub struct Reporting {
    /// `ReportEnable`.
    pub enabled: bool,
    /// `FirstContact`.
    pub first_contact: bool,
    /// `StartedGroundRun`.
    pub started_ground_run: bool,
    /// `LandingReported`.
    pub landing_reported: bool,
    /// `TakeoffReported`.
    pub takeoff_reported: bool,
    /// `SinkRate`, ft/s.
    pub sink_rate: f64,
    /// `GroundSpeed`, ft/s.
    pub ground_speed: f64,
    /// `TakeoffDistanceTraveled`, ft.
    pub takeoff_distance: f64,
    /// `TakeoffDistanceTraveled50ft`, ft.
    pub takeoff_distance_50ft: f64,
    /// `LandingDistanceTraveled`, ft.
    pub landing_distance: f64,
    /// `MaximumStrutForce`, lbs.
    pub max_strut_force: f64,
    /// `MaximumStrutTravel`, ft.
    pub max_strut_travel: f64,
}

impl Default for Reporting {
    fn default() -> Self {
        Self {
            enabled: true,
            first_contact: false,
            started_ground_run: false,
            landing_reported: false,
            takeoff_reported: false,
            sink_rate: 0.0,
            ground_speed: 0.0,
            takeoff_distance: 0.0,
            takeoff_distance_50ft: 0.0,
            landing_distance: 0.0,
            max_strut_force: 0.0,
            max_strut_travel: 0.0,
        }
    }
}

impl Reporting {
    /// The part of `ResetToIC` that reporting owns.
    pub fn reset_to_ic(&mut self) {
        self.first_contact = false;
        self.started_ground_run = false;
        self.landing_distance = 0.0;
        self.takeoff_distance = 0.0;
        self.takeoff_distance_50ft = 0.0;
        self.max_strut_force = 0.0;
        self.max_strut_travel = 0.0;
        self.sink_rate = 0.0;
        self.ground_speed = 0.0;
    }
}

crate::snapshot::visit_unit_enum! { FrictionType { Roll, Side, Dynamic } }
crate::snapshot::visit_fields! { Multipliers { roll, side, dynamic } }
crate::snapshot::visit_fields! {
    ContactForce {
        t_ground, ground_normal, ground_wheel_vel, normal_force, body_force, moment, brake_coeff,
        static_friction, multipliers,
    }
}
crate::snapshot::visit_fields! {
    Reporting {
        enabled, first_contact, started_ground_run, landing_reported, takeoff_reported, sink_rate,
        ground_speed, takeoff_distance, takeoff_distance_50ft, landing_distance, max_strut_force,
        max_strut_travel,
    }
}
