// SPDX-License-Identifier: LGPL-2.1-or-later
//! The subset of the `libm` crate's API that the port calls, answered by
//! the platform maths library, which C++ JSBSim calls. Patched in by
//! scripts/oracle-sincos.sh so that the port and an oracle built without
//! sin/cos fusion can be compared bit for bit.

mod platform;

pub use platform::*;
