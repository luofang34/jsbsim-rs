// SPDX-License-Identifier: LGPL-2.1-or-later
// FGJSBBase: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/FGJSBBase.h (axis index enums).
//
//! Vector, matrix, quaternion and location types with JSBSim's
//! `FGColumnVector3`, `FGMatrix33`, `FGQuaternion` and `FGLocation`
//! semantics. Arithmetic follows the C++ operation order and
//! transcendental functions come from `libm`, so every target computes the
//! same bits. Indices are 1-based, as in JSBSim.

mod location;
mod matrix33;
mod quaternion;
mod vector3;

#[cfg(test)]
mod tests;

pub use location::Location;
pub use matrix33::Matrix33;
pub use quaternion::Quaternion;
pub use vector3::{ColumnVector3, dot_product};

/// Roll moment / x axis index (`eL`, `eX`, `eP`, `eU`, `eRoll`).
pub const E_L: usize = 1;
/// Pitch moment index (`eM`).
pub const E_M: usize = 2;
/// Yaw moment index (`eN`).
pub const E_N: usize = 3;
/// Roll rate index (`eP`).
pub const E_P: usize = 1;
/// Pitch rate index (`eQ`).
pub const E_Q: usize = 2;
/// Yaw rate index (`eR`).
pub const E_R: usize = 3;
/// Body x velocity index (`eU`).
pub const E_U: usize = 1;
/// Body y velocity index (`eV`).
pub const E_V: usize = 2;
/// Body z velocity index (`eW`).
pub const E_W: usize = 3;
/// x index (`eX`).
pub const E_X: usize = 1;
/// y index (`eY`).
pub const E_Y: usize = 2;
/// z index (`eZ`).
pub const E_Z: usize = 3;
/// Roll angle index (`ePhi`).
pub const E_PHI: usize = 1;
/// Pitch angle index (`eTht`).
pub const E_THT: usize = 2;
/// Heading index (`ePsi`).
pub const E_PSI: usize = 3;
/// Drag index (`eDrag`).
pub const E_DRAG: usize = 1;
/// Side force index (`eSide`).
pub const E_SIDE: usize = 2;
/// Lift index (`eLift`).
pub const E_LIFT: usize = 3;
/// Roll index (`eRoll`).
pub const E_ROLL: usize = 1;
/// Pitch index (`ePitch`).
pub const E_PITCH: usize = 2;
/// Yaw index (`eYaw`).
pub const E_YAW: usize = 3;
/// North index (`eNorth`).
pub const E_NORTH: usize = 1;
/// East index (`eEast`).
pub const E_EAST: usize = 2;
/// Down index (`eDown`).
pub const E_DOWN: usize = 3;
/// Latitude index (`eLat`).
pub const E_LAT: usize = 1;
/// Longitude index (`eLong`).
pub const E_LONG: usize = 2;
/// Radius index (`eRad`).
pub const E_RAD: usize = 3;
