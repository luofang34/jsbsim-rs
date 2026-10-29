// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Tony Peden.
// Derived from JSBSim src/models/propulsion/FGForce.{h,cpp}.
//
//! A force with its point of application and orientation (`FGForce`),
//! resolved into body-frame force and moment about the CG; every thruster
//! is one.

use crate::math::{ColumnVector3, E_PITCH, E_ROLL, E_YAW, Matrix33};
use crate::models::external_reactions::Transforms;
use crate::models::mass_balance::MassBalance;

/// `FGForce::TransformType`: the frame the native force is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformType {
    /// `tNone`: body axes.
    None,
    /// `tWindBody`.
    WindBody,
    /// `tLocalBody`.
    LocalBody,
    /// `tInertialBody`.
    InertialBody,
    /// `tCustom`: the orientation angles' own transform.
    Custom,
}

/// `FGForce`.
#[derive(Debug, Clone, PartialEq)]
pub struct Force {
    /// Native force (`vFn`), lbs.
    pub native_force: ColumnVector3,
    /// Native moment (`vMn`), lbs·ft.
    pub native_moment: ColumnVector3,
    /// Orientation to the body: roll, pitch, yaw, rad (`vOrient`).
    pub orient: ColumnVector3,
    /// Frame of the native force.
    pub transform_type: TransformType,
    /// Location, structural frame, in (`vXYZn`).
    pub location: ColumnVector3,
    /// Point the force acts at, in (`vActingXYZn`).
    pub acting_location: ColumnVector3,
    /// Custom transform (`mT`).
    pub transform: Matrix33,
    /// Body-frame force of the last [`Force::body_forces`] (`vFb`).
    pub body_force: ColumnVector3,
    /// Moment about the CG of the last [`Force::body_forces`] (`vM`).
    pub moment: ColumnVector3,
}

impl Default for Force {
    /// `FGForce()`: no transform.
    fn default() -> Self {
        Self {
            native_force: ColumnVector3::zero(),
            native_moment: ColumnVector3::zero(),
            orient: ColumnVector3::zero(),
            transform_type: TransformType::None,
            location: ColumnVector3::zero(),
            acting_location: ColumnVector3::zero(),
            transform: Matrix33::new(1., 0., 0., 0., 1., 0., 0., 0., 1.),
            body_force: ColumnVector3::zero(),
            moment: ColumnVector3::zero(),
        }
    }
}

impl Force {
    /// `Transform`.
    #[must_use]
    pub fn transform(&self, t: &Transforms) -> Matrix33 {
        match self.transform_type {
            TransformType::WindBody => t.tw2b,
            TransformType::LocalBody => t.tl2b,
            TransformType::InertialBody => t.ti2b,
            TransformType::Custom | TransformType::None => self.transform,
        }
    }

    /// `GetBodyForces`: the body-frame force, and the moment about the CG
    /// of `mass_balance` it makes at the acting location.
    pub fn body_forces(&mut self, t: &Transforms, mass_balance: &MassBalance) -> ColumnVector3 {
        self.body_force = self.transform(t) * self.native_force;
        let dxyz = mass_balance.structural_to_body(self.acting_location);
        self.moment = self.native_moment + dxyz.cross(self.body_force);
        self.body_force
    }

    /// `SetLocation`, which also moves the acting location.
    pub fn set_location(&mut self, location: ColumnVector3) {
        self.location = location;
        self.acting_location = location;
    }

    /// `UpdateCustomTransformMatrix`.
    pub fn update_custom_transform_matrix(&mut self) {
        let o = &self.orient;
        let (sp, cp) = libm::sincos(o[E_PITCH]);
        let (sr, cr) = libm::sincos(o[E_ROLL]);
        let (sy, cy) = libm::sincos(o[E_YAW]);
        let srsp = sr * sp;
        let crcy = cr * cy;
        let crsy = cr * sy;
        self.transform = Matrix33::new(
            cp * cy,
            srsp * cy - crsy,
            crcy * sp + sr * sy,
            cp * sy,
            srsp * sy + crcy,
            crsy * sp - sr * cy,
            -sp,
            sr * cp,
            cr * cp,
        );
    }

    /// `SetAnglesToBody`: only a custom transform takes angles.
    pub fn set_angles_to_body(&mut self, angles: ColumnVector3) {
        if self.transform_type == TransformType::Custom {
            self.orient = angles;
            self.update_custom_transform_matrix();
        }
    }

    /// `SetPitch`.
    pub fn set_pitch(&mut self, pitch: f64) {
        self.orient[E_PITCH] = pitch;
        self.update_custom_transform_matrix();
    }

    /// `SetYaw`.
    pub fn set_yaw(&mut self, yaw: f64) {
        self.orient[E_YAW] = yaw;
        self.update_custom_transform_matrix();
    }
}
