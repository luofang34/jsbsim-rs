// SPDX-License-Identifier: LGPL-2.1-or-later
// FGJSBBase: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/FGJSBBase.h (axis index enums).
//
//! Vector, matrix, quaternion and location types with JSBSim's
//! `FGColumnVector3`, `FGMatrix33`, `FGQuaternion` and `FGLocation`
//! semantics. Arithmetic follows the C++ operation order and
//! transcendental functions come from `libm`, so every target computes the
//! same bits. Components are named by [`Axis`] (and the `E_*` aliases of
//! JSBSim's axis enums), so no index can fall outside a vector or matrix.

mod axis;
mod location;
mod matrix33;
mod quaternion;
mod vector3;

#[cfg(test)]
mod tests;

pub use axis::Axis;
pub use location::Location;
pub use matrix33::Matrix33;
pub use quaternion::Quaternion;
pub use vector3::{ColumnVector3, dot_product};

/// Roll moment / x axis (`eL`, `eX`, `eP`, `eU`, `eRoll`).
pub const E_L: Axis = Axis::X;
/// Pitch moment axis (`eM`).
pub const E_M: Axis = Axis::Y;
/// Yaw moment axis (`eN`).
pub const E_N: Axis = Axis::Z;
/// Roll rate axis (`eP`).
pub const E_P: Axis = Axis::X;
/// Pitch rate axis (`eQ`).
pub const E_Q: Axis = Axis::Y;
/// Yaw rate axis (`eR`).
pub const E_R: Axis = Axis::Z;
/// Body x velocity axis (`eU`).
pub const E_U: Axis = Axis::X;
/// Body y velocity axis (`eV`).
pub const E_V: Axis = Axis::Y;
/// Body z velocity axis (`eW`).
pub const E_W: Axis = Axis::Z;
/// x axis (`eX`).
pub const E_X: Axis = Axis::X;
/// y axis (`eY`).
pub const E_Y: Axis = Axis::Y;
/// z axis (`eZ`).
pub const E_Z: Axis = Axis::Z;
/// Roll angle axis (`ePhi`).
pub const E_PHI: Axis = Axis::X;
/// Pitch angle axis (`eTht`).
pub const E_THT: Axis = Axis::Y;
/// Heading axis (`ePsi`).
pub const E_PSI: Axis = Axis::Z;
/// Drag axis (`eDrag`).
pub const E_DRAG: Axis = Axis::X;
/// Side force axis (`eSide`).
pub const E_SIDE: Axis = Axis::Y;
/// Lift axis (`eLift`).
pub const E_LIFT: Axis = Axis::Z;
/// Roll axis (`eRoll`).
pub const E_ROLL: Axis = Axis::X;
/// Pitch axis (`ePitch`).
pub const E_PITCH: Axis = Axis::Y;
/// Yaw axis (`eYaw`).
pub const E_YAW: Axis = Axis::Z;
/// North axis (`eNorth`).
pub const E_NORTH: Axis = Axis::X;
/// East axis (`eEast`).
pub const E_EAST: Axis = Axis::Y;
/// Down axis (`eDown`).
pub const E_DOWN: Axis = Axis::Z;
/// Latitude axis (`eLat`).
pub const E_LAT: Axis = Axis::X;
/// Longitude axis (`eLong`).
pub const E_LONG: Axis = Axis::Y;
/// Radius axis (`eRad`).
pub const E_RAD: Axis = Axis::Z;

/// Tracing target of [`uncached_derived_read`].
pub const UNCACHED_DERIVED_TARGET: &str = "jsbsim::uncached_derived";

/// A derived value (Euler angles, rotation matrices, geodetic position) read
/// through `&self` from a [`Location`] or [`Quaternion`] whose owner did not
/// call `cache_derived` after changing it, so the read computes the values
/// again. Only this miss path reports it, at trace level; a test counts these
/// events over steady-state frames and requires none.
#[cold]
fn uncached_derived_read(kind: &'static str) {
    tracing::trace!(target: "jsbsim::uncached_derived", kind);
}
