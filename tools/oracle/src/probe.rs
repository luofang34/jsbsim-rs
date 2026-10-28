// SPDX-License-Identifier: LGPL-2.1-or-later
//! Rust counterparts of the C++ driver's probe modes. Each interpreter reads
//! the same input as its `tools/oracle/cpp/probe_*.cpp` twin and must print
//! the same transcript.

mod atmosphere;
mod functions;
mod mass_balance;
mod math;
mod properties;
mod table;
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
        "math" => Ok(math::run(input)),
        "functions" => Ok(functions::run(input, context)),
        "table" => Ok(table::run(input, context)),
        "xml" => Ok(xml::run(input, context)),
        "mass-balance" => Ok(mass_balance::run(input, context)),
        "atmosphere" => atmosphere::run(input).map_err(|source| OracleError::Probe {
            probe: "atmosphere",
            source,
        }),
        other => Err(OracleError::Provenance(format!("unknown probe {other}"))),
    }
}
