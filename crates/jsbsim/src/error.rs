// SPDX-License-Identifier: LGPL-2.1-or-later
//! Error type shared by the whole crate, and the errors it wraps.

use thiserror::Error as ThisError;

pub use crate::functions::FunctionError;
pub use crate::properties::PathError;
pub use crate::xml::{SourceLocation, XmlError};

#[cfg(test)]
mod tests;

/// Errors produced while loading or running a simulation.
#[derive(Debug, ThisError, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The resolver has no resource at the requested path.
    #[error("resource not found: {path}")]
    ResourceNotFound {
        /// Path that was requested from the resolver.
        path: String,
    },
    /// The input uses a construct this port does not support, or one
    /// JSBSim does not know under strict loading. Loading fails rather than
    /// silently ignoring it.
    #[error("{at}: unsupported {construct}")]
    Unsupported {
        /// Element or attribute that is not supported, e.g. `<child>`.
        construct: String,
        /// Where it appeared.
        at: SourceLocation,
    },
    /// Malformed XML or an invalid value in it.
    #[error(transparent)]
    Xml(#[from] XmlError),
    /// A table, function or condition that is invalid or fails to evaluate.
    #[error(transparent)]
    Function(#[from] FunctionError),
    /// An `<input>` whose `type` JSBSim has no input class for (`NONE`
    /// among them), which fails the whole load in JSBSim.
    #[error("{at}: no input type for <input type=\"{kind}\">")]
    UnknownInputType {
        /// The `type` attribute as written.
        kind: String,
        /// Where it appeared.
        at: SourceLocation,
    },
    /// A model configuration or runtime state JSBSim refuses.
    #[error("{}", located_message(.at.as_ref(), .message))]
    Model {
        /// Where in the XML input the offending element starts, when the
        /// refusal comes from an element.
        at: Option<SourceLocation>,
        /// What is wrong, without a location prefix.
        message: String,
    },
    /// `TrimFailureException`: a trim run by
    /// [`Fdm::do_trim`](crate::Fdm::do_trim),
    /// [`Fdm::do_trim_custom`](crate::Fdm::do_trim_custom) or
    /// `simulation/do_simple_trim` did not converge ("Trim Failed"), or
    /// `simulation/do_simple_trim` named no trim mode ("Illegal trimming
    /// mode!").
    #[error("{message}")]
    TrimFailed {
        /// JSBSim's message.
        message: String,
    },
    /// `GeographicLib::GeographicErr`: an ellipsoid GeographicLib refuses,
    /// such as a planet with a radius that is not positive.
    #[error("{message}")]
    Geographic {
        /// GeographicLib's message.
        message: String,
    },
    /// A snapshot that does not decode or does not fit this executive.
    #[error(transparent)]
    Snapshot(#[from] crate::snapshot::SnapshotError),
    /// A property path SimGear would reject. Its message includes the
    /// [`PathError`]'s, which is therefore not also given as its source.
    #[error("invalid property path: {0}")]
    PropertyPath(PathError),
    /// The resolver found something at the path but could not read it,
    /// such as a directory or a file without read permission.
    #[error("cannot read {path}: {kind}")]
    ResourceUnreadable {
        /// Path that was requested from the resolver.
        path: String,
        /// Why the read failed.
        kind: std::io::ErrorKind,
    },
}

/// `{at}: {message}`, or the bare message without a location.
fn located_message(at: Option<&SourceLocation>, message: &str) -> String {
    match at {
        Some(at) => format!("{at}: {message}"),
        None => message.to_owned(),
    }
}

impl Error {
    /// An [`Error::Model`] no element of the input is responsible for.
    pub(crate) fn model(message: impl Into<String>) -> Self {
        Self::Model {
            at: None,
            message: message.into(),
        }
    }

    /// An [`Error::Model`] raised by the element at `at`.
    pub(crate) fn model_at(at: SourceLocation, message: impl Into<String>) -> Self {
        Self::Model {
            at: Some(at),
            message: message.into(),
        }
    }

    /// Where in the XML input the error arose, for the errors that know.
    #[must_use]
    pub fn location(&self) -> Option<&SourceLocation> {
        match self {
            Self::Unsupported { at, .. } | Self::UnknownInputType { at, .. } => Some(at),
            Self::Model { at, .. } => at.as_ref(),
            Self::Xml(e) => e.location(),
            Self::Function(e) => e.location(),
            Self::ResourceNotFound { .. }
            | Self::TrimFailed { .. }
            | Self::Geographic { .. }
            | Self::Snapshot(_)
            | Self::PropertyPath(_)
            | Self::ResourceUnreadable { .. } => None,
        }
    }
}

impl From<PathError> for Error {
    fn from(e: PathError) -> Self {
        Self::PropertyPath(e)
    }
}
