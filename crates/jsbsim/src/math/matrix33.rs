// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org)
// Copyright (C) 1998 by the authors above
// Derived from JSBSim src/math/FGMatrix33.{h,cpp}.
//
//! 3x3 matrix, stored column-major like JSBSim so every expression reads the
//! same elements in the same order.

use std::f64::consts::PI;
use std::ops::{Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Sub, SubAssign};

use super::{Axis, ColumnVector3, Quaternion};

/// `FGMatrix33`; entries are indexed `(row, col)` by [`Axis`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Matrix33 {
    /// Column-major: entry `(row, col)` is `data[3 * col + row]`, counting
    /// axes from 0.
    data: [f64; 9],
}

impl Matrix33 {
    /// A matrix from its entries given row by row.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        m11: f64,
        m12: f64,
        m13: f64,
        m21: f64,
        m22: f64,
        m23: f64,
        m31: f64,
        m32: f64,
        m33: f64,
    ) -> Self {
        Self {
            data: [m11, m21, m31, m12, m22, m32, m13, m23, m33],
        }
    }

    /// The zero matrix (`FGMatrix33()`).
    #[must_use]
    pub const fn zero() -> Self {
        Self { data: [0.0; 9] }
    }

    /// `Transposed()`.
    #[must_use]
    pub fn transposed(&self) -> Self {
        let d = &self.data;
        Self::new(d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7], d[8])
    }

    /// `T()`: transposes in place.
    pub fn transpose(&mut self) {
        self.data.swap(3, 1);
        self.data.swap(6, 2);
        self.data.swap(7, 5);
    }

    /// `Determinant()`.
    #[must_use]
    pub fn determinant(&self) -> f64 {
        let d = &self.data;
        d[0] * d[4] * d[8] + d[3] * d[7] * d[2] + d[6] * d[1] * d[5]
            - d[6] * d[4] * d[2]
            - d[3] * d[1] * d[8]
            - d[7] * d[5] * d[0]
    }

    /// `Invertible()`: exact singularity test.
    #[must_use]
    pub fn invertible(&self) -> bool {
        self.determinant() != 0.0
    }

    /// `Inverse()` by Cramer's rule; the zero matrix when singular.
    #[must_use]
    pub fn inverse(&self) -> Self {
        if self.determinant() == 0.0 {
            return Self::zero();
        }
        let d = &self.data;
        let rdet = 1.0 / self.determinant();
        Self::new(
            rdet * (d[4] * d[8] - d[7] * d[5]),
            rdet * (d[6] * d[5] - d[3] * d[8]),
            rdet * (d[3] * d[7] - d[6] * d[4]),
            rdet * (d[7] * d[2] - d[1] * d[8]),
            rdet * (d[0] * d[8] - d[6] * d[2]),
            rdet * (d[6] * d[1] - d[0] * d[7]),
            rdet * (d[1] * d[5] - d[4] * d[2]),
            rdet * (d[3] * d[2] - d[0] * d[5]),
            rdet * (d[0] * d[4] - d[3] * d[1]),
        )
    }

    /// `GetQuaternion()`: the quaternion of this rotation matrix, from the
    /// largest of the four candidate pivots.
    #[must_use]
    pub fn quaternion(&self) -> Quaternion {
        let d = &self.data;
        let temp = [
            1.0 + d[0] + d[4] + d[8],
            1.0 + d[0] - d[4] - d[8],
            1.0 - d[0] + d[4] - d[8],
            1.0 - d[0] - d[4] + d[8],
        ];
        // The first of equal maxima wins, as in JSBSim's strict `>` scan.
        let mut idx = 0;
        let mut max = temp[0];
        for (i, &t) in temp.iter().enumerate().skip(1) {
            if t > max {
                (idx, max) = (i, t);
            }
        }
        let mut q = [0.0; 4];
        match idx {
            0 => {
                q[0] = 0.50 * temp[0].sqrt();
                q[1] = 0.25 * (d[7] - d[5]) / q[0];
                q[2] = 0.25 * (d[2] - d[6]) / q[0];
                q[3] = 0.25 * (d[3] - d[1]) / q[0];
            }
            1 => {
                q[1] = 0.50 * temp[1].sqrt();
                q[0] = 0.25 * (d[7] - d[5]) / q[1];
                q[2] = 0.25 * (d[3] + d[1]) / q[1];
                q[3] = 0.25 * (d[2] + d[6]) / q[1];
            }
            2 => {
                q[2] = 0.50 * temp[2].sqrt();
                q[0] = 0.25 * (d[2] - d[6]) / q[2];
                q[1] = 0.25 * (d[3] + d[1]) / q[2];
                q[3] = 0.25 * (d[7] + d[5]) / q[2];
            }
            _ => {
                q[3] = 0.50 * temp[3].sqrt();
                q[0] = 0.25 * (d[3] - d[1]) / q[3];
                q[1] = 0.25 * (d[6] + d[2]) / q[3];
                q[2] = 0.25 * (d[7] + d[5]) / q[3];
            }
        }
        Quaternion::from_components(q[0], q[1], q[2], q[3])
    }

    /// `GetEuler()`: (phi, theta, psi) in radians, psi in [0, 2pi); at
    /// gimbal lock psi is 0.
    #[must_use]
    pub fn euler(&self) -> ColumnVector3 {
        let d = &self.data;
        let mut euler = ColumnVector3::zero();
        let gimbal_lock = if d[6] <= -1.0 {
            euler[Axis::Y] = 0.5 * PI;
            true
        } else if 1.0 <= d[6] {
            euler[Axis::Y] = -0.5 * PI;
            true
        } else {
            euler[Axis::Y] = libm::asin(-d[6]);
            false
        };
        if gimbal_lock {
            euler[Axis::X] = libm::atan2(-d[5], d[4]);
            euler[Axis::Z] = 0.0;
        } else {
            euler[Axis::X] = libm::atan2(d[7], d[8]);
            let mut psi = libm::atan2(d[3], d[0]);
            if psi < 0.0 {
                psi += 2.0 * PI;
            }
            euler[Axis::Z] = psi;
        }
        euler
    }
}

impl Div<f64> for Matrix33 {
    type Output = Self;
    /// `operator/(scalar)`: multiplication by `1/scalar`.
    fn div(mut self, scalar: f64) -> Self {
        self /= scalar;
        self
    }
}

impl DivAssign<f64> for Matrix33 {
    /// `operator/=(scalar)`: multiplication by `1/scalar`.
    fn div_assign(&mut self, scalar: f64) {
        let tmp = 1.0 / scalar;
        for x in &mut self.data {
            *x *= tmp;
        }
    }
}

impl Index<(Axis, Axis)> for Matrix33 {
    type Output = f64;
    fn index(&self, (row, col): (Axis, Axis)) -> &f64 {
        let [m11, m21, m31, m12, m22, m32, m13, m23, m33] = &self.data;
        match (row, col) {
            (Axis::X, Axis::X) => m11,
            (Axis::Y, Axis::X) => m21,
            (Axis::Z, Axis::X) => m31,
            (Axis::X, Axis::Y) => m12,
            (Axis::Y, Axis::Y) => m22,
            (Axis::Z, Axis::Y) => m32,
            (Axis::X, Axis::Z) => m13,
            (Axis::Y, Axis::Z) => m23,
            (Axis::Z, Axis::Z) => m33,
        }
    }
}

impl IndexMut<(Axis, Axis)> for Matrix33 {
    fn index_mut(&mut self, (row, col): (Axis, Axis)) -> &mut f64 {
        let [m11, m21, m31, m12, m22, m32, m13, m23, m33] = &mut self.data;
        match (row, col) {
            (Axis::X, Axis::X) => m11,
            (Axis::Y, Axis::X) => m21,
            (Axis::Z, Axis::X) => m31,
            (Axis::X, Axis::Y) => m12,
            (Axis::Y, Axis::Y) => m22,
            (Axis::Z, Axis::Y) => m32,
            (Axis::X, Axis::Z) => m13,
            (Axis::Y, Axis::Z) => m23,
            (Axis::Z, Axis::Z) => m33,
        }
    }
}

impl Mul<ColumnVector3> for Matrix33 {
    type Output = ColumnVector3;
    fn mul(self, v: ColumnVector3) -> ColumnVector3 {
        let d = &self.data;
        let [v1, v2, v3] = v.to_array();
        let mut tmp1 = v1 * d[0];
        let mut tmp2 = v1 * d[1];
        let mut tmp3 = v1 * d[2];
        tmp1 += v2 * d[3];
        tmp2 += v2 * d[4];
        tmp3 += v2 * d[5];
        tmp1 += v3 * d[6];
        tmp2 += v3 * d[7];
        tmp3 += v3 * d[8];
        ColumnVector3::new(tmp1, tmp2, tmp3)
    }
}

impl Mul for Matrix33 {
    type Output = Self;
    fn mul(self, m: Self) -> Self {
        let a = &self.data;
        // One column of the product from that column of `m`, summed in
        // JSBSim's order.
        let column = |[b1, b2, b3]: [f64; 3]| {
            [
                a[0] * b1 + a[3] * b2 + a[6] * b3,
                a[1] * b1 + a[4] * b2 + a[7] * b3,
                a[2] * b1 + a[5] * b2 + a[8] * b3,
            ]
        };
        let [b11, b21, b31, b12, b22, b32, b13, b23, b33] = m.data;
        let [p11, p21, p31] = column([b11, b21, b31]);
        let [p12, p22, p32] = column([b12, b22, b32]);
        let [p13, p23, p33] = column([b13, b23, b33]);
        Self {
            data: [p11, p21, p31, p12, p22, p32, p13, p23, p33],
        }
    }
}

impl MulAssign for Matrix33 {
    fn mul_assign(&mut self, m: Self) {
        *self = *self * m;
    }
}

impl Mul<f64> for Matrix33 {
    type Output = Self;
    fn mul(mut self, scalar: f64) -> Self {
        for x in &mut self.data {
            *x *= scalar;
        }
        self
    }
}

impl Mul<Matrix33> for f64 {
    type Output = Matrix33;
    fn mul(self, m: Matrix33) -> Matrix33 {
        m * self
    }
}

impl MulAssign<f64> for Matrix33 {
    fn mul_assign(&mut self, scalar: f64) {
        for x in &mut self.data {
            *x *= scalar;
        }
    }
}

impl Add for Matrix33 {
    type Output = Self;
    fn add(mut self, m: Self) -> Self {
        self += m;
        self
    }
}

impl AddAssign for Matrix33 {
    fn add_assign(&mut self, m: Self) {
        for (x, y) in self.data.iter_mut().zip(m.data) {
            *x += y;
        }
    }
}

impl Sub for Matrix33 {
    type Output = Self;
    fn sub(mut self, m: Self) -> Self {
        self -= m;
        self
    }
}

impl SubAssign for Matrix33 {
    fn sub_assign(&mut self, m: Self) {
        for (x, y) in self.data.iter_mut().zip(m.data) {
            *x -= y;
        }
    }
}
