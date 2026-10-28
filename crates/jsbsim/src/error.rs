// SPDX-License-Identifier: LGPL-2.1-or-later
//! Error type shared by the whole crate.

use thiserror::Error as ThisError;

use crate::functions::FunctionError;
use crate::properties::PathError;
use crate::xml::XmlError;

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
        /// Resource in which it appeared, with its line when known.
        path: String,
    },
    /// Malformed XML or an invalid value in it.
    #[error(transparent)]
    Xml(#[from] XmlError),
    /// A table, function or condition that is invalid or fails to evaluate.
    #[error(transparent)]
    Function(#[from] FunctionError),
    /// A model configuration JSBSim refuses to load.
    #[error("{message}")]
    Model {
        /// What is wrong, with the location.
        message: String,
    },
    /// A property path SimGear would reject.
    #[error("invalid property path: {0}")]
    PropertyPath(#[from] PathError),
}
