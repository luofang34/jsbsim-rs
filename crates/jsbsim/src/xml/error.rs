// SPDX-License-Identifier: LGPL-2.1-or-later
// FGXMLElement: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/input_output/FGXMLElement.cpp and
// string_utilities.cpp (the errors they throw).
//
//! Errors of the XML layer. Each names the file, line and element involved,
//! like JSBSim's `XMLLogException` messages.

use std::fmt;

use thiserror::Error;

/// Where in the input an element starts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Location {
    /// Resource path of the document.
    pub file: String,
    /// One-based line of the element's start tag.
    pub line: u32,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "In file {}: line {}", self.file, self.line)
    }
}

/// Error raised while reading or querying XML.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum XmlError {
    /// The document is not well-formed XML.
    #[error("{file}:{line}:{column}: {message}")]
    Malformed {
        /// Resource path of the document.
        file: String,
        /// One-based line.
        line: u32,
        /// One-based column.
        column: u32,
        /// Parser message.
        message: String,
    },
    /// The document's declared encoding is not supported, or its bytes do
    /// not match it.
    #[error("{file}: cannot decode as {encoding}")]
    Encoding {
        /// Resource path of the document.
        file: String,
        /// Declared encoding.
        encoding: String,
    },
    /// Text that should be a number is not one.
    #[error("{at}: <{element}>: {message}")]
    InvalidNumber {
        /// Element holding the text.
        element: String,
        /// Where the element is.
        at: Location,
        /// JSBSim's message, which quotes the text.
        message: String,
    },
    /// A numeric element has no data.
    #[error("{at}: <{element}>: Expected numeric value, but got no data")]
    NoData {
        /// Element without data.
        element: String,
        /// Where the element is.
        at: Location,
    },
    /// A numeric element has more than one data line.
    #[error("{at}: Attempting to get single data value in element <{element}> from {lines} lines")]
    MultipleLines {
        /// Element with several lines.
        element: String,
        /// Where the element is.
        at: Location,
        /// Number of data lines.
        lines: usize,
    },
    /// A required attribute is absent or empty.
    #[error("{at}: <{element}>: Expecting numeric attribute value, but got no data ({attribute})")]
    MissingAttribute {
        /// Element that lacks it.
        element: String,
        /// Attribute name.
        attribute: String,
        /// Where the element is.
        at: Location,
    },
    /// A required child element is absent.
    #[error("{at}: <{parent}>: Attempting to get non-existent element {name}")]
    MissingElement {
        /// Element that was searched.
        parent: String,
        /// Child that was not found.
        name: String,
        /// Where the parent is.
        at: Location,
    },
    /// A `unit` attribute names a unit JSBSim does not know.
    #[error("{at}: Supplied unit: \"{unit}\" does not exist (typo?).")]
    UnknownUnit {
        /// The unit.
        unit: String,
        /// Where the element is.
        at: Location,
    },
    /// A unit is known but cannot be converted to the required one.
    #[error("{at}: Supplied unit: \"{from}\" cannot be converted to {to}")]
    Inconvertible {
        /// Supplied unit.
        from: String,
        /// Required unit.
        to: String,
        /// Where the element is.
        at: Location,
    },
    /// An included file could not be found or read.
    #[error("{at}: Could not open file: {file}")]
    IncludeNotFound {
        /// Value of the `file` attribute.
        file: String,
        /// Where the including element is.
        at: Location,
    },
}
