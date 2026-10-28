// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1998 Tony Peden and Jon S. Berndt (jon@jsbsim.org)
// Copyright (C) 2001 by Tony Peden and Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/math/FGColumnVector3.{h,cpp}.
//
//! Three-component column vector.

use std::ops::{
    Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign,
};

use super::Axis;

/// `FGColumnVector3`; entries are indexed by [`Axis`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ColumnVector3 {
    data: [f64; 3],
}

impl ColumnVector3 {
    /// A vector with the given entries.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { data: [x, y, z] }
    }

    /// The zero vector.
    #[must_use]
    pub const fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    /// Entries as an array, in order.
    #[must_use]
    pub const fn to_array(self) -> [f64; 3] {
        self.data
    }

    /// `InitMatrix(a, b, c)`.
    pub fn set(&mut self, x: f64, y: f64, z: f64) {
        self.data = [x, y, z];
    }

    /// `Magnitude()`: the Euclidean norm.
    #[must_use]
    pub fn magnitude(&self) -> f64 {
        let [x, y, z] = self.data;
        (x * x + y * y + z * z).sqrt()
    }

    /// `Magnitude(idx1, idx2)`: the norm projected on two axes.
    #[must_use]
    pub fn magnitude2(&self, idx1: Axis, idx2: Axis) -> f64 {
        let (a, b) = (self[idx1], self[idx2]);
        (a * a + b * b).sqrt()
    }

    /// `Normalize()`: scales to unit length; a zero vector is unchanged.
    pub fn normalize(&mut self) -> &mut Self {
        let mag = self.magnitude();
        if mag != 0.0 {
            *self *= 1.0 / mag;
        }
        self
    }

    /// Cross product (`operator*` between vectors).
    #[must_use]
    pub fn cross(self, v: Self) -> Self {
        let [a0, a1, a2] = self.data;
        let [b0, b1, b2] = v.data;
        Self::new(a1 * b2 - a2 * b1, a2 * b0 - a0 * b2, a0 * b1 - a1 * b0)
    }
}

/// `DotProduct(v1, v2)`.
#[must_use]
pub fn dot_product(v1: ColumnVector3, v2: ColumnVector3) -> f64 {
    let ([a1, a2, a3], [b1, b2, b3]) = (v1.data, v2.data);
    a1 * b1 + a2 * b2 + a3 * b3
}

impl Index<Axis> for ColumnVector3 {
    type Output = f64;
    fn index(&self, axis: Axis) -> &f64 {
        let [x, y, z] = &self.data;
        match axis {
            Axis::X => x,
            Axis::Y => y,
            Axis::Z => z,
        }
    }
}

impl IndexMut<Axis> for ColumnVector3 {
    fn index_mut(&mut self, axis: Axis) -> &mut f64 {
        let [x, y, z] = &mut self.data;
        match axis {
            Axis::X => x,
            Axis::Y => y,
            Axis::Z => z,
        }
    }
}

impl Mul<f64> for ColumnVector3 {
    type Output = Self;
    fn mul(self, scalar: f64) -> Self {
        let [x, y, z] = self.data;
        Self::new(scalar * x, scalar * y, scalar * z)
    }
}

impl Mul<ColumnVector3> for f64 {
    type Output = ColumnVector3;
    fn mul(self, v: ColumnVector3) -> ColumnVector3 {
        v * self
    }
}

impl MulAssign<f64> for ColumnVector3 {
    fn mul_assign(&mut self, scalar: f64) {
        for x in &mut self.data {
            *x *= scalar;
        }
    }
}

impl Add for ColumnVector3 {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self::new(
            self.data[0] + b.data[0],
            self.data[1] + b.data[1],
            self.data[2] + b.data[2],
        )
    }
}

impl Sub for ColumnVector3 {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self::new(
            self.data[0] - b.data[0],
            self.data[1] - b.data[1],
            self.data[2] - b.data[2],
        )
    }
}

impl AddAssign for ColumnVector3 {
    fn add_assign(&mut self, b: Self) {
        *self = *self + b;
    }
}

impl SubAssign for ColumnVector3 {
    fn sub_assign(&mut self, b: Self) {
        *self = *self - b;
    }
}

impl Neg for ColumnVector3 {
    type Output = Self;
    /// `-v`, which JSBSim writes as `-1.0*v`.
    fn neg(self) -> Self {
        self * -1.0
    }
}

impl Div<f64> for ColumnVector3 {
    type Output = Self;
    /// `operator/`: multiplication by `1/scalar`; division by zero logs an
    /// error and yields the zero vector.
    fn div(self, scalar: f64) -> Self {
        if scalar != 0.0 {
            return self * (1.0 / scalar);
        }
        let [x, y, z] = self.data;
        tracing::error!(
            "Attempt to divide by zero in method FGColumnVector3::operator/(const double scalar), object {x} , {y} , {z}"
        );
        Self::zero()
    }
}

impl DivAssign<f64> for ColumnVector3 {
    /// `operator/=`: in place; division by zero logs and leaves the vector.
    fn div_assign(&mut self, scalar: f64) {
        if scalar != 0.0 {
            *self *= 1.0 / scalar;
        } else {
            let [x, y, z] = self.data;
            tracing::error!(
                "Attempt to divide by zero in method FGColumnVector3::operator/=(const double scalar), object {x} , {y} , {z}"
            );
        }
    }
}
