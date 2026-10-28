// SPDX-License-Identifier: LGPL-2.1-or-later
//! Rust counterparts of the C++ driver's probe modes. Each interpreter reads
//! the same input as its `tools/oracle/cpp/probe_*.cpp` twin and must print
//! the same transcript.

mod properties;
mod units;
mod xml;

use std::path::PathBuf;

use crate::OracleError;

/// Where a probe finds the files its input names.
#[derive(Debug, Clone, Default)]
pub struct ProbeContext {
    /// Directory of the input file, for `local` paths.
    pub input_dir: PathBuf,
    /// JSBSim checkout, for `file` paths; absent in `cargo test`.
    pub jsbsim_root: Option<PathBuf>,
}

/// Runs the Rust side of probe `name` on `input`.
pub fn run(name: &str, input: &str, context: &ProbeContext) -> Result<String, OracleError> {
    match name {
        "properties" => Ok(properties::run(input)),
        "units" => Ok(units::run(input)),
        "xml" => Ok(xml::run(input, context)),
        other => Err(OracleError::Provenance(format!("unknown probe {other}"))),
    }
}
