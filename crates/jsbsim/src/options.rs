// SPDX-License-Identifier: LGPL-2.1-or-later
//! How strictly inputs are loaded.

/// Options for loading models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct LoadOptions {
    /// Turn every input that JSBSim only logs and then ignores (an unknown
    /// operation, an unknown template, a one-argument variadic operation,
    /// ...) into [`FunctionError::Strict`](crate::error::FunctionError::Strict).
    /// Off by default, which matches JSBSim.
    pub strict: bool,
}

impl LoadOptions {
    /// The default options, which load as JSBSim does.
    #[must_use]
    pub const fn new() -> Self {
        Self { strict: false }
    }

    /// These options with [`strict`](Self::strict) set to `strict`.
    #[must_use]
    pub const fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }
}
