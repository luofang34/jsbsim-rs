// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGMassBalance.h (struct PointMass).
//
//! A point mass: weight, location and the inertia of its own shape.

use crate::math::{ColumnVector3, Matrix33};
use crate::units::SLUGTOLB;

/// `PointMass::esShape`. A cylinder and a ball are solid, a tube and a
/// sphere hollow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shape {
    /// No `<form>`: the inertia is read from the point mass element, or
    /// a `<form>` whose shape JSBSim does not know, which has none.
    #[default]
    Unspecified,
    /// Hollow cylinder.
    Tube,
    /// Solid cylinder.
    Cylinder,
    /// Hollow sphere.
    Sphere,
    /// Solid sphere.
    Ball,
}

/// `FGMassBalance::PointMass`.
#[derive(Debug, Clone, Default)]
pub struct PointMass {
    /// Shape, which decides the inertia `calculate_shape_inertia` gives.
    pub shape: Shape,
    /// Location, structural frame, in.
    pub location: ColumnVector3,
    /// Weight, lbs.
    pub weight: f64,
    /// Radius, ft.
    pub radius: f64,
    /// Length, ft.
    pub length: f64,
    /// The `name` attribute.
    pub name: String,
    /// Inertia about its own centre, slug·ft^2.
    pub inertia: Matrix33,
}

impl PointMass {
    /// `PointMass(w, vXYZ)`.
    #[must_use]
    pub fn new(weight: f64, location: ColumnVector3) -> Self {
        Self {
            weight,
            location,
            ..Self::default()
        }
    }

    /// `CalculateShapeInertia`: only the diagonal of a shaped point mass
    /// is set; an unspecified shape keeps its inertia.
    pub fn calculate_shape_inertia(&mut self) {
        let (w, r, l) = (self.weight, self.radius, self.length);
        let m = &mut self.inertia;
        match self.shape {
            Shape::Tube => {
                m[(1, 1)] = (w / SLUGTOLB) * r * r;
                m[(2, 2)] = (w / (SLUGTOLB * 12.0)) * (6.0 * r * r + l * l);
                m[(3, 3)] = m[(2, 2)];
            }
            Shape::Cylinder => {
                m[(1, 1)] = (w / (SLUGTOLB * 2.0)) * r * r;
                m[(2, 2)] = (w / (SLUGTOLB * 12.0)) * (3.0 * r * r + l * l);
                m[(3, 3)] = m[(2, 2)];
            }
            Shape::Sphere => {
                m[(1, 1)] = (w / (SLUGTOLB * 3.0)) * r * r * 2.0;
                m[(2, 2)] = m[(1, 1)];
                m[(3, 3)] = m[(1, 1)];
            }
            Shape::Ball => {
                m[(1, 1)] = (w / (SLUGTOLB * 5.0)) * r * r * 2.0;
                m[(2, 2)] = m[(1, 1)];
                m[(3, 3)] = m[(1, 1)];
            }
            Shape::Unspecified => {}
        }
    }

    /// `SetPointMassWeight`: the shape inertia follows the weight.
    pub fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
        self.calculate_shape_inertia();
    }

    /// `SetPointMassLocation(axis, value)`, `axis` one-based.
    pub fn set_location(&mut self, axis: usize, value: f64) {
        self.location[axis] = value;
    }
}
