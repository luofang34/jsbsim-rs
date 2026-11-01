// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/math/FGLocation.h (the arithmetic operators).
//
//! Arithmetic on the ECEF coordinates. Each operator invalidates the derived
//! values: a binary operator builds a new location, with the ellipse of its
//! left operand, and an assignment keeps the target's ellipse.

use std::ops::{Add, AddAssign, DivAssign, Mul, MulAssign, Sub, SubAssign};

use super::Location;

impl Add for Location {
    type Output = Self;
    /// `operator+`.
    fn add(self, l: Self) -> Self {
        self.with_ellipse_of(self.ec_loc + l.ec_loc)
    }
}

impl Sub for Location {
    type Output = Self;
    /// `operator-`.
    fn sub(self, l: Self) -> Self {
        self.with_ellipse_of(self.ec_loc - l.ec_loc)
    }
}

impl Mul<f64> for Location {
    type Output = Self;
    /// `operator*(scalar)`.
    fn mul(self, scalar: f64) -> Self {
        self.with_ellipse_of(scalar * self.ec_loc)
    }
}

impl Mul<Location> for f64 {
    type Output = Location;
    /// `operator*(scalar, location)`.
    fn mul(self, l: Location) -> Location {
        l * self
    }
}

impl AddAssign for Location {
    /// `operator+=`.
    fn add_assign(&mut self, l: Self) {
        self.ec_loc += l.ec_loc;
        self.invalidate();
    }
}

impl SubAssign for Location {
    /// `operator-=`.
    fn sub_assign(&mut self, l: Self) {
        self.ec_loc -= l.ec_loc;
        self.invalidate();
    }
}

impl MulAssign<f64> for Location {
    /// `operator*=`.
    fn mul_assign(&mut self, scalar: f64) {
        self.ec_loc *= scalar;
        self.invalidate();
    }
}

impl DivAssign<f64> for Location {
    /// `operator/=`: scaling by `1/scalar`.
    fn div_assign(&mut self, scalar: f64) {
        *self *= 1.0 / scalar;
    }
}
