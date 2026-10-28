// SPDX-License-Identifier: LGPL-2.1-or-later
//! Reference oracle for the JSBSim port.
//!
//! Builds the C++ JSBSim out of tree, runs a script through a small driver
//! that samples properties at full precision, stores the results as golden
//! CSVs with a provenance manifest, and compares candidate output against a
//! golden using a per-property tolerance table.

pub mod cases;
pub mod cli;
pub mod compare;
pub mod corpus;
pub mod derived;
pub mod error;
pub mod manifest;
pub mod nasa;
pub mod port_script;
pub mod probe;
pub mod series;
pub mod stats;
pub mod tolerance;
pub mod transcript;
pub mod upstream;

pub use compare::{Divergence, Outcome, compare};
pub use error::OracleError;
pub use manifest::Manifest;
pub use series::Series;
pub use tolerance::{Tolerance, ToleranceTable};
