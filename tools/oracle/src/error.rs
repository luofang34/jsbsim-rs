// SPDX-License-Identifier: LGPL-2.1-or-later
//! Error type of the oracle.

use std::path::PathBuf;

use thiserror::Error;

/// Everything that can go wrong while building, running or comparing.
#[derive(Debug, Error)]
pub enum OracleError {
    /// Reading or writing a file failed.
    #[error("{action} {path}: {source}")]
    Io {
        /// What was being done, e.g. "reading".
        action: &'static str,
        /// File involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// A CSV file is malformed.
    #[error("{source_name}:{line}: {message}")]
    Csv {
        /// File name or label of the CSV.
        source_name: String,
        /// One-based line number.
        line: usize,
        /// What is wrong.
        message: String,
    },
    /// A TOML file could not be parsed.
    #[error("parsing {path}: {source}")]
    Toml {
        /// File that failed to parse.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: Box<toml::de::Error>,
    },
    /// A value could not be serialised to TOML.
    #[error("serialising {path}: {source}")]
    TomlWrite {
        /// File that was being written.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: toml::ser::Error,
    },
    /// A tolerance row is invalid.
    #[error("tolerance row {index} ({case} {property}): {message}")]
    Tolerance {
        /// Zero-based index of the row in the table.
        index: usize,
        /// Case the row belongs to.
        case: String,
        /// Property the row applies to.
        property: String,
        /// What is wrong.
        message: String,
    },
    /// A compared column has no tolerance row covering the sample time.
    #[error("case {case}: no tolerance row covers {property} at t={time}")]
    MissingTolerance {
        /// Case being compared.
        case: String,
        /// Column without a tolerance.
        property: String,
        /// Sample time that is not covered.
        time: f64,
    },
    /// Reference and candidate do not log the same columns.
    #[error("column mismatch: reference has {reference:?}, candidate has {candidate:?}")]
    ColumnMismatch {
        /// Reference header.
        reference: Vec<String>,
        /// Candidate header.
        candidate: Vec<String>,
    },
    /// An external program could not be started or failed.
    #[error("{program} {status}: {detail}")]
    Command {
        /// Program that was run.
        program: String,
        /// Exit status or spawn failure.
        status: String,
        /// Captured stderr or context.
        detail: String,
    },
    /// The Rust side of a probe failed.
    #[error("probe {probe}: {source}")]
    Probe {
        /// Probe name.
        probe: &'static str,
        /// What failed.
        #[source]
        source: jsbsim::Error,
    },
    /// The upstream checkout or a manifest is not in the expected state.
    #[error("{0}")]
    Provenance(String),
    /// NASA check-case data that is missing, unreachable or not the data
    /// the manifest pins.
    #[error("NASA data {archive}: {message}")]
    NasaData {
        /// Archive name from `oracle/nasa/manifest.toml`.
        archive: String,
        /// What is wrong.
        message: String,
    },
    /// Invalid command-line usage.
    #[error(transparent)]
    Usage(#[from] clap::Error),
}

impl OracleError {
    /// Builds an [`OracleError::Io`].
    pub fn io(action: &'static str, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }
}
