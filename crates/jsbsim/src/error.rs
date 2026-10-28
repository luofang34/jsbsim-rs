// SPDX-License-Identifier: LGPL-2.1-or-later
//! Error type shared by the whole crate.

use thiserror::Error as ThisError;

/// Errors produced while loading or running a simulation.
#[derive(Debug, ThisError, Clone, PartialEq, Eq)]
pub enum Error {
    /// The resolver has no resource at the requested path.
    #[error("resource not found: {path}")]
    ResourceNotFound {
        /// Path that was requested from the resolver.
        path: String,
    },
    /// The input uses a JSBSim construct this port does not implement yet.
    /// Loading fails rather than silently ignoring it.
    #[error("unsupported {construct} in {path}")]
    Unsupported {
        /// Element or attribute that is not supported, e.g. `<rotor>`.
        construct: String,
        /// Resource in which it appeared.
        path: String,
    },
}
