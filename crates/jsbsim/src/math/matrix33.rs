// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org)
// Copyright (C) 1998 by the authors above
// Derived from JSBSim src/math/FGMatrix33.{h,cpp}.
//
//! 3x3 matrix, stored column-major like JSBSim so every expression reads the
//! same elements in the same order.

use std::f64::consts::PI;
use std::ops::{Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Sub, SubAssign};

use super::{ColumnVector3, Quaternion};

/// `FGMatrix33`; entries are indexed `(row, col)` from 1.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Matrix33 {
    /// Column-major: `data[(col-1)*3 + row-1]`.
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
        let mut idx = 0;
        for i in 1..4 {
            if temp[i] > temp[idx] {
                idx = i;
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
            euler[2] = 0.5 * PI;
            true
        } else if 1.0 <= d[6] {
            euler[2] = -0.5 * PI;
            true
        } else {
            euler[2] = libm::asin(-d[6]);
            false
        };
        if gimbal_lock {
            euler[1] = libm::atan2(-d[5], d[4]);
            euler[3] = 0.0;
        } else {
            euler[1] = libm::atan2(d[7], d[8]);
            let mut psi = libm::atan2(d[3], d[0]);
            if psi < 0.0 {
                psi += 2.0 * PI;
            }
            euler[3] = psi;
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

impl Index<(usize, usize)> for Matrix33 {
    type Output = f64;
    fn index(&self, (row, col): (usize, usize)) -> &f64 {
        &self.data[(col - 1) * 3 + row - 1]
    }
}

impl IndexMut<(usize, usize)> for Matrix33 {
    fn index_mut(&mut self, (row, col): (usize, usize)) -> &mut f64 {
        &mut self.data[(col - 1) * 3 + row - 1]
    }
}

impl Mul<ColumnVector3> for Matrix33 {
    type Output = ColumnVector3;
    fn mul(self, v: ColumnVector3) -> ColumnVector3 {
        let d = &self.data;
        let (v1, v2, v3) = (v[1], v[2], v[3]);
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
        let (a, b) = (&self.data, &m.data);
        let mut p = [0.0; 9];
        for row in 0..3 {
            for col in 0..3 {
                p[col * 3 + row] =
                    a[row] * b[col * 3] + a[3 + row] * b[col * 3 + 1] + a[6 + row] * b[col * 3 + 2];
            }
        }
        Self { data: p }
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
