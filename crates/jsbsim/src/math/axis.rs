// SPDX-License-Identifier: LGPL-2.1-or-later
// FGJSBBase: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/FGJSBBase.h (axis index enums).
//
//! Typed axis of a three-component vector or matrix, so an out-of-range
//! component cannot be named.

/// One of the three axes JSBSim numbers 1 to 3 (`eX`, `eY`, `eZ` and
/// their aliases such as `eP`, `eU`, `ePhi`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Axis {
    /// The first axis (JSBSim index 1).
    X,
    /// The second axis (JSBSim index 2).
    Y,
    /// The third axis (JSBSim index 3).
    Z,
}

impl Axis {
    /// The axes in JSBSim order.
    pub const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    /// JSBSim's 1-based index of this axis.
    #[must_use]
    pub const fn index1(self) -> usize {
        match self {
            Self::X => 1,
            Self::Y => 2,
            Self::Z => 3,
        }
    }

    /// The axis JSBSim numbers `i` (1 to 3); `None` for any other `i`.
    #[must_use]
    pub const fn from_index1(i: usize) -> Option<Self> {
        match i {
            1 => Some(Self::X),
            2 => Some(Self::Y),
            3 => Some(Self::Z),
            _ => None,
        }
    }
}
