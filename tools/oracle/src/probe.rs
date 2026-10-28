// SPDX-License-Identifier: LGPL-2.1-or-later
//! Rust counterparts of the C++ driver's probe modes. Each interpreter reads
//! the same input as its `tools/oracle/cpp/probe_*.cpp` twin and must print
//! the same transcript.

mod properties;

use crate::OracleError;

/// Runs the Rust side of probe `name` on `input`.
pub fn run(name: &str, input: &str) -> Result<String, OracleError> {
    match name {
        "properties" => Ok(properties::run(input)),
        other => Err(OracleError::Provenance(format!("unknown probe {other}"))),
    }
}
