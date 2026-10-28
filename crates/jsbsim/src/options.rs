// SPDX-License-Identifier: LGPL-2.1-or-later
//! How strictly inputs are loaded.

/// Options for loading models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoadOptions {
    /// Turn every input that JSBSim only logs and then ignores (an unknown
    /// operation, an unknown template, a one-argument variadic operation,
    /// ...) into [`FunctionError::Strict`](crate::functions::FunctionError::Strict).
    /// Off by default, which matches JSBSim.
    pub strict: bool,
}
