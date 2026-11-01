// SPDX-License-Identifier: LGPL-2.1-or-later
// FGJSBBase: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/FGJSBBase.h (axis index enums).
//
//! Vector, matrix, quaternion and location types with JSBSim's
//! `FGColumnVector3`, `FGMatrix33`, `FGQuaternion` and `FGLocation`
//! semantics. Arithmetic follows the C++ operation order and
//! transcendental functions come from `libm`, so every target computes the
//! same bits. Components are named by [`Axis`], so no index can fall outside
//! a vector or matrix.

mod axis;
mod column_vector3;
mod geodesic;
mod location;
mod matrix33;
mod quaternion;

#[cfg(test)]
mod tests;

pub use axis::Axis;
pub use column_vector3::ColumnVector3;
pub use location::Location;
pub use matrix33::Matrix33;
pub use quaternion::Quaternion;

/// Roll moment axis (`eL`).
pub(crate) const E_L: Axis = Axis::X;
/// Pitch moment axis (`eM`).
pub(crate) const E_M: Axis = Axis::Y;
/// Yaw moment axis (`eN`).
pub(crate) const E_N: Axis = Axis::Z;
/// Roll rate axis (`eP`).
pub(crate) const E_P: Axis = Axis::X;
/// Pitch rate axis (`eQ`).
pub(crate) const E_Q: Axis = Axis::Y;
/// Yaw rate axis (`eR`).
pub(crate) const E_R: Axis = Axis::Z;
/// Body x velocity axis (`eU`).
pub(crate) const E_U: Axis = Axis::X;
/// Body y velocity axis (`eV`).
pub(crate) const E_V: Axis = Axis::Y;
/// Body z velocity axis (`eW`).
pub(crate) const E_W: Axis = Axis::Z;
/// Roll angle axis (`ePhi`).
pub(crate) const E_PHI: Axis = Axis::X;
/// Pitch angle axis (`eTht`).
pub(crate) const E_THT: Axis = Axis::Y;
/// Heading axis (`ePsi`).
pub(crate) const E_PSI: Axis = Axis::Z;
/// Drag axis (`eDrag`).
pub(crate) const E_DRAG: Axis = Axis::X;
/// Side force axis (`eSide`).
pub(crate) const E_SIDE: Axis = Axis::Y;
/// Lift axis (`eLift`).
pub(crate) const E_LIFT: Axis = Axis::Z;
/// Roll axis (`eRoll`).
pub(crate) const E_ROLL: Axis = Axis::X;
/// Pitch axis (`ePitch`).
pub(crate) const E_PITCH: Axis = Axis::Y;
/// Yaw axis (`eYaw`).
pub(crate) const E_YAW: Axis = Axis::Z;
/// North axis (`eNorth`).
pub(crate) const E_NORTH: Axis = Axis::X;
/// East axis (`eEast`).
pub(crate) const E_EAST: Axis = Axis::Y;
/// Down axis (`eDown`).
pub(crate) const E_DOWN: Axis = Axis::Z;

/// `x²`, spelled as the C++ source spells it (`sqr`).
#[must_use]
pub(crate) const fn sqr(x: f64) -> f64 {
    x * x
}

/// Tracing target of the event a [`Location`] or [`Quaternion`] emits when
/// a derived value is read without its owner having called
/// `cache_derived`, so the read computes it again.
pub(crate) const UNCACHED_DERIVED_TARGET: &str = "jsbsim::uncached_derived";

/// A derived value (Euler angles, rotation matrices, geodetic position) read
/// through `&self` from a [`Location`] or [`Quaternion`] whose owner did not
/// call `cache_derived` after changing it, so the read computes the values
/// again. Only this miss path reports it, at trace level; a test counts these
/// events over steady-state frames and requires none.
#[cold]
fn uncached_derived_read(kind: &'static str) {
    tracing::trace!(target: UNCACHED_DERIVED_TARGET, kind);
}
