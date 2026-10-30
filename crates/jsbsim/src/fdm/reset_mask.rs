// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.h (ResetToInitialConditions mode flags).
//
//! The mode bits of [`Fdm::reset_to_initial_conditions`](crate::Fdm::reset_to_initial_conditions).

use std::ops::{BitOr, BitOrAssign};

#[cfg(test)]
mod tests;

/// The `mode` of JSBSim's `ResetToInitialConditions(int mode)`: a set of
/// flag bits.
///
/// The bits stay private so flags JSBSim adds later can join as new
/// constants without breaking anyone. A mask may carry bits no constant
/// names: [`from_bits_retain`](Self::from_bits_retain) keeps them, as an
/// `int` written to `simulation/reset` would, and the reset ignores them as
/// JSBSim does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ResetMask(u32);

impl ResetMask {
    /// `START_NEW_OUTPUT` (0x1): JSBSim tells its outputs to close their
    /// files and open new ones. The port's outputs write nothing, so the
    /// flag is accepted and has no effect.
    pub const START_NEW_OUTPUT: Self = Self(0x1);

    /// `DONT_EXECUTE_RUN_IC` (0x2): the reset leaves out `RunIC`, so the
    /// caller can adjust state, such as control surface deflections the
    /// reset would have overwritten, before calling
    /// [`Fdm::run_ic`](crate::Fdm::run_ic) itself.
    pub const DONT_EXECUTE_RUN_IC: Self = Self(0x2);

    /// No flags: a full reset that runs `RunIC`.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// The mask with exactly `bits`, unnamed ones included, as JSBSim
    /// receives an `int` mode from `simulation/reset`.
    #[must_use]
    pub const fn from_bits_retain(bits: u32) -> Self {
        Self(bits)
    }

    /// The raw bits.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Whether every flag of `other` is set here.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for ResetMask {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ResetMask {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
