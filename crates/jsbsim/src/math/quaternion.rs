// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/math/FGQuaternion.{h,cpp}.
//
//! Unit quaternion for orientations, with lazily computed transformation
//! matrices and Euler angles.

use std::cell::OnceCell;
use std::ops::{Add, AddAssign, Index, Mul, MulAssign, Sub, SubAssign};

use super::{ColumnVector3, E_P, E_PHI, E_PSI, E_Q, E_R, E_THT, Matrix33};
use crate::units::RADTODEG;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Derived {
    t: Matrix33,
    t_inv: Matrix33,
    euler: ColumnVector3,
    sines: ColumnVector3,
    cosines: ColumnVector3,
}

/// `FGQuaternion`; components are indexed 1 to 4.
#[derive(Debug, Clone)]
pub struct Quaternion {
    data: [f64; 4],
    derived: OnceCell<Derived>,
}

impl PartialEq for Quaternion {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl Default for Quaternion {
    /// The identity rotation.
    fn default() -> Self {
        Self::from_components(1.0, 0.0, 0.0, 0.0)
    }
}

impl Quaternion {
    /// A quaternion with the given components, not normalized.
    #[must_use]
    pub fn from_components(q1: f64, q2: f64, q3: f64, q4: f64) -> Self {
        Self {
            data: [q1, q2, q3, q4],
            derived: OnceCell::new(),
        }
    }

    /// `zero()`.
    #[must_use]
    pub fn zero() -> Self {
        Self::from_components(0.0, 0.0, 0.0, 0.0)
    }

    /// From Euler angles (3-2-1 sequence), normalized.
    #[must_use]
    pub fn from_euler(phi: f64, tht: f64, psi: f64) -> Self {
        let (thtd2, psid2, phid2) = (0.5 * tht, 0.5 * psi, 0.5 * phi);
        let (sthtd2, spsid2, sphid2) = (libm::sin(thtd2), libm::sin(psid2), libm::sin(phid2));
        let (cthtd2, cpsid2, cphid2) = (libm::cos(thtd2), libm::cos(psid2), libm::cos(phid2));
        let cphid2_cthtd2 = cphid2 * cthtd2;
        let cphid2_sthtd2 = cphid2 * sthtd2;
        let sphid2_sthtd2 = sphid2 * sthtd2;
        let sphid2_cthtd2 = sphid2 * cthtd2;
        let mut q = Self::from_components(
            cphid2_cthtd2 * cpsid2 + sphid2_sthtd2 * spsid2,
            sphid2_cthtd2 * cpsid2 - cphid2_sthtd2 * spsid2,
            cphid2_sthtd2 * cpsid2 + sphid2_cthtd2 * spsid2,
            cphid2_cthtd2 * spsid2 - sphid2_sthtd2 * cpsid2,
        );
        q.normalize();
        q
    }

    /// From an Euler angle vector (phi, theta, psi).
    #[must_use]
    pub fn from_euler_vector(orient: ColumnVector3) -> Self {
        Self::from_euler(orient[E_PHI], orient[E_THT], orient[E_PSI])
    }

    /// A rotation of `angle` about axis `idx` (`ePhi`, `eTht` or `ePsi`).
    #[must_use]
    pub fn from_axis_index(idx: usize, angle: f64) -> Self {
        let angle2 = 0.5 * angle;
        let (s, c) = (libm::sin(angle2), libm::cos(angle2));
        match idx {
            E_PHI => Self::from_components(c, s, 0.0, 0.0),
            E_THT => Self::from_components(c, 0.0, s, 0.0),
            _ => Self::from_components(c, 0.0, 0.0, s),
        }
    }

    /// A rotation of `angle` about an arbitrary `axis`.
    #[must_use]
    pub fn from_angle_axis(angle: f64, axis: ColumnVector3) -> Self {
        let angle2 = 0.5 * angle;
        let length = axis.magnitude();
        let s = libm::sin(angle2) / length;
        let c = libm::cos(angle2);
        Self::from_components(c, s * axis[1], s * axis[2], s * axis[3])
    }

    /// From a transformation matrix, normalized (`FGQuaternion(m)`).
    #[must_use]
    pub fn from_matrix(m: &Matrix33) -> Self {
        let q0 = 0.50 * (1.0 + m[(1, 1)] + m[(2, 2)] + m[(3, 3)]).sqrt();
        let t = 0.25 / q0;
        let mut q = Self::from_components(
            q0,
            t * (m[(2, 3)] - m[(3, 2)]),
            t * (m[(3, 1)] - m[(1, 3)]),
            t * (m[(1, 2)] - m[(2, 1)]),
        );
        q.normalize();
        q
    }

    /// Components as an array.
    #[must_use]
    pub fn to_array(&self) -> [f64; 4] {
        self.data
    }

    /// Sets component `idx` (1-based), invalidating derived values.
    pub fn set(&mut self, idx: usize, value: f64) {
        self.data[idx - 1] = value;
        self.derived = OnceCell::new();
    }

    /// `GetQDot(PQR)`: the time derivative for body rates `pqr`.
    #[must_use]
    pub fn q_dot(&self, pqr: ColumnVector3) -> Self {
        let d = &self.data;
        let (p, q, r) = (pqr[E_P], pqr[E_Q], pqr[E_R]);
        Self::from_components(
            -0.5 * (d[1] * p + d[2] * q + d[3] * r),
            0.5 * (d[0] * p - d[3] * q + d[2] * r),
            0.5 * (d[3] * p + d[0] * q - d[1] * r),
            0.5 * (-d[2] * p + d[1] * q + d[0] * r),
        )
    }

    /// `SqrMagnitude()`.
    #[must_use]
    pub fn sqr_magnitude(&self) -> f64 {
        let d = &self.data;
        d[0] * d[0] + d[1] * d[1] + d[2] * d[2] + d[3] * d[3]
    }

    /// `Magnitude()`.
    #[must_use]
    pub fn magnitude(&self) -> f64 {
        self.sqr_magnitude().sqrt()
    }

    /// `Normalize()`: left untouched when zero or within 1e-10 of unit
    /// length.
    pub fn normalize(&mut self) {
        let norm = self.magnitude();
        if norm == 0.0 || (norm - 1.000).abs() < 1e-10 {
            return;
        }
        *self *= 1.0 / norm;
    }

    /// `Inverse()`: the conjugate divided by the squared norm; a zero
    /// quaternion is returned unchanged.
    #[must_use]
    pub fn inverse(&self) -> Self {
        let norm = self.sqr_magnitude();
        if norm == 0.0 {
            return self.clone();
        }
        let r = 1.0 / norm;
        let d = &self.data;
        Self::from_components(d[0] * r, -d[1] * r, -d[2] * r, -d[3] * r)
    }

    /// `Conjugate()`.
    #[must_use]
    pub fn conjugate(&self) -> Self {
        let d = &self.data;
        Self::from_components(d[0], -d[1], -d[2], -d[3])
    }

    /// `QExp(omega)`: the quaternion exponential of `omega`.
    #[must_use]
    pub fn exp(omega: ColumnVector3) -> Self {
        let angle = omega.magnitude();
        let sina_a = if angle > 0.0 {
            libm::sin(angle) / angle
        } else {
            1.0
        };
        Self::from_components(
            libm::cos(angle),
            omega[1] * sina_a,
            omega[2] * sina_a,
            omega[3] * sina_a,
        )
    }

    fn derived(&self) -> &Derived {
        self.derived.get_or_init(|| compute_derived(&self.data))
    }

    /// `GetT()`: the transformation matrix (local to body for an attitude).
    #[must_use]
    pub fn t(&self) -> Matrix33 {
        self.derived().t
    }

    /// `GetTInv()`: the transpose of [`t`](Self::t).
    #[must_use]
    pub fn t_inv(&self) -> Matrix33 {
        self.derived().t_inv
    }

    /// `GetEuler()`: (phi, theta, psi) in radians.
    #[must_use]
    pub fn euler(&self) -> ColumnVector3 {
        self.derived().euler
    }

    /// `GetEulerDeg()`.
    #[must_use]
    pub fn euler_deg(&self) -> ColumnVector3 {
        RADTODEG * self.derived().euler
    }

    /// `GetSinEuler(i)`.
    #[must_use]
    pub fn sin_euler(&self, i: usize) -> f64 {
        self.derived().sines[i]
    }

    /// `GetCosEuler(i)`.
    #[must_use]
    pub fn cos_euler(&self, i: usize) -> f64 {
        self.derived().cosines[i]
    }
}

fn compute_derived(d: &[f64; 4]) -> Derived {
    let [q0, q1, q2, q3] = *d;
    let (q0q0, q1q1, q2q2, q3q3) = (q0 * q0, q1 * q1, q2 * q2, q3 * q3);
    let (q0q1, q0q2, q0q3) = (q0 * q1, q0 * q2, q0 * q3);
    let (q1q2, q1q3, q2q3) = (q1 * q2, q1 * q3, q2 * q3);
    let t = Matrix33::new(
        q0q0 + q1q1 - q2q2 - q3q3,
        2.0 * (q1q2 + q0q3),
        2.0 * (q1q3 - q0q2),
        2.0 * (q1q2 - q0q3),
        q0q0 - q1q1 + q2q2 - q3q3,
        2.0 * (q2q3 + q0q1),
        2.0 * (q1q3 + q0q2),
        2.0 * (q2q3 - q0q1),
        q0q0 - q1q1 - q2q2 + q3q3,
    );
    let euler = t.euler();
    Derived {
        t,
        t_inv: t.transposed(),
        sines: ColumnVector3::new(libm::sin(euler[E_PHI]), -t[(1, 3)], libm::sin(euler[E_PSI])),
        cosines: ColumnVector3::new(
            libm::cos(euler[E_PHI]),
            libm::cos(euler[E_THT]),
            libm::cos(euler[E_PSI]),
        ),
        euler,
    }
}

impl Index<usize> for Quaternion {
    type Output = f64;
    fn index(&self, idx: usize) -> &f64 {
        &self.data[idx - 1]
    }
}

impl Add for &Quaternion {
    type Output = Quaternion;
    fn add(self, q: Self) -> Quaternion {
        let (a, b) = (&self.data, &q.data);
        Quaternion::from_components(a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3])
    }
}

impl Sub for &Quaternion {
    type Output = Quaternion;
    fn sub(self, q: Self) -> Quaternion {
        let (a, b) = (&self.data, &q.data);
        Quaternion::from_components(a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3])
    }
}

impl AddAssign<&Quaternion> for Quaternion {
    fn add_assign(&mut self, q: &Quaternion) {
        *self = &*self + q;
    }
}

impl SubAssign<&Quaternion> for Quaternion {
    fn sub_assign(&mut self, q: &Quaternion) {
        *self = &*self - q;
    }
}

impl Mul for &Quaternion {
    type Output = Quaternion;
    /// Hamilton product.
    fn mul(self, q: Self) -> Quaternion {
        let (a, b) = (&self.data, &q.data);
        Quaternion::from_components(
            a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
            a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
            a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
            a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
        )
    }
}

impl MulAssign<&Quaternion> for Quaternion {
    fn mul_assign(&mut self, q: &Quaternion) {
        *self = &*self * q;
    }
}

impl Mul<f64> for &Quaternion {
    type Output = Quaternion;
    fn mul(self, scalar: f64) -> Quaternion {
        let d = &self.data;
        Quaternion::from_components(scalar * d[0], scalar * d[1], scalar * d[2], scalar * d[3])
    }
}

impl Mul<&Quaternion> for f64 {
    type Output = Quaternion;
    fn mul(self, q: &Quaternion) -> Quaternion {
        q * self
    }
}

impl MulAssign<f64> for Quaternion {
    fn mul_assign(&mut self, scalar: f64) {
        for x in &mut self.data {
            *x *= scalar;
        }
        self.derived = OnceCell::new();
    }
}
