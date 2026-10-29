// SPDX-License-Identifier: LGPL-2.1-or-later
//! `oracle determinism`: golden cases of the Rust port recorded bit for
//! bit, so that a native run and a wasm32 run of the port can be compared
//! (`scripts/wasm-determinism.sh`).
//!
//! Native code and both wasm32 targets compute the same doubles because:
//!
//! - rustc lowers `f64` addition, subtraction, multiplication, division
//!   and `sqrt` to the correctly rounded IEEE-754 binary64 operations on
//!   every target, and never reassociates them or contracts `a * b + c`
//!   into a fused multiply-add. One source gives one sequence of rounded
//!   operations, whatever the code generator.
//! - Every inexact function (`sin`, `exp`, `log`, `pow`, `atan2`, ...)
//!   comes from the pure-Rust `libm` crate, compiled from the same source
//!   for every target, never from a platform libm or a target intrinsic.
//!   Where `libm` itself uses a hardware instruction it is one whose
//!   result IEEE-754 fixes exactly (`sqrt`, `fma`, `rint`, `floor`,
//!   `ceil`, `trunc`, `fabs`).
//! - `wasm32-wasip1` and `wasm32-unknown-unknown` share the wasm32 code
//!   generator and differ only in the system interface, which the `jsbsim`
//!   core does not use since it does no I/O. The WASI target stands in for
//!   the browser one here because it can read the inputs and write records.
//!
//! What wasm leaves open is the sign and payload of a NaN result, so a NaN
//! is compared by its bits like any other value and a NaN mismatch is
//! reported as one.
//!
//! A record holds bits, not decimals, so that no float formatting or
//! parsing sits between the two runs: a script case is written to
//! `<case>.bits`, the series header and then one line per sample of
//! `{:016x}` `f64` bits; a transcript case to `<case>.txt`, verbatim.

mod diff;
#[cfg(test)]
mod tests;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use serde::Deserialize;
use tracing::info;

use crate::manifest::{self, CaseSpec, INPUT_FILE, Manifest};
use crate::{OracleError, Series};

pub use diff::{Mismatch, Outcome, compare_dirs_blocking, compare_records};

/// The case list, relative to the repository root.
pub const CASES_FILE: &str = "oracle/determinism.toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseList {
    cases: Vec<String>,
}

/// The cases of [`CASES_FILE`], in order.
pub fn read_cases_blocking(repo_root: &Path) -> Result<Vec<String>, OracleError> {
    let path = repo_root.join(CASES_FILE);
    let text = std::fs::read_to_string(&path).map_err(|e| OracleError::io("reading", &path, e))?;
    let list: CaseList = toml::from_str(&text).map_err(|e| OracleError::Toml {
        path,
        source: Box::new(e),
    })?;
    Ok(list.cases)
}

/// The `determinism` subcommand and its `dump` and `compare` actions.
#[must_use]
pub fn command() -> Command {
    let cases = Arg::new("case")
        .action(ArgAction::Append)
        .help("Golden case names; those of oracle/determinism.toml when omitted");
    let dir = |name: &'static str, help: &'static str| {
        Arg::new(name)
            .required(true)
            .value_parser(value_parser!(PathBuf))
            .help(help)
    };
    Command::new("determinism")
        .about("Record golden cases bit for bit, or compare two sets of records")
        .subcommand_required(true)
        .subcommand(
            Command::new("dump")
                .about("Run the Rust port on cases and write one record per case")
                .arg(
                    Arg::new("out")
                        .long("out")
                        .required(true)
                        .value_parser(value_parser!(PathBuf))
                        .help("Directory to write the records to"),
                )
                .arg(cases.clone()),
        )
        .subcommand(
            Command::new("compare")
                .about("Require two record directories to hold the same bits")
                .arg(dir("native", "Records of the native run"))
                .arg(dir("wasm", "Records of the wasm run"))
                .arg(cases),
        )
}

/// Runs the `determinism` action selected in `matches`.
pub fn run_blocking(repo_root: &Path, matches: &ArgMatches) -> Result<ExitCode, OracleError> {
    let Some((name, sub)) = matches.subcommand() else {
        return Ok(ExitCode::FAILURE);
    };
    let named: Vec<String> = sub
        .get_many::<String>("case")
        .map(|values| values.cloned().collect())
        .unwrap_or_default();
    let cases = if named.is_empty() {
        read_cases_blocking(repo_root)?
    } else {
        named
    };
    let dir = |name: &str| sub.get_one::<PathBuf>(name).cloned().unwrap_or_default();
    match name {
        "dump" => dump_blocking(repo_root, &dir("out"), &cases).map(|()| ExitCode::SUCCESS),
        "compare" => compare_dirs_blocking(&dir("native"), &dir("wasm"), &cases),
        _ => Ok(ExitCode::FAILURE),
    }
}

/// What a case records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record {
    /// A script case's series as bits.
    Bits(String),
    /// A transcript case's output.
    Transcript(String),
}

impl Record {
    /// File name of `case`'s record.
    #[must_use]
    pub fn file_name(&self, case: &str) -> String {
        match self {
            Self::Bits(_) => format!("{case}.bits"),
            Self::Transcript(_) => format!("{case}.txt"),
        }
    }

    /// The record's text.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Bits(text) | Self::Transcript(text) => text,
        }
    }
}

/// Runs the Rust port on golden case `case` with the vendored inputs, as
/// `oracle port` does. A script run that stops with an error fails.
pub fn record_blocking(repo_root: &Path, case: &str) -> Result<Record, OracleError> {
    let dir = manifest::case_dir(&repo_root.join("oracle").join("goldens"), case);
    let manifest = Manifest::read_blocking(&dir)?;
    match &manifest.case {
        CaseSpec::Script(spec) => {
            let (series, stop) = crate::port_script::run_script(&spec.rust_root(repo_root), spec);
            match stop {
                Some(stop) => Err(OracleError::Determinism {
                    case: case.to_owned(),
                    message: format!(
                        "the Rust run stopped after {} rows: {stop}",
                        series.rows.len()
                    ),
                }),
                None => Ok(Record::Bits(bits_text(&series))),
            }
        }
        CaseSpec::Transcript(spec) => {
            let input_path = dir.join(INPUT_FILE);
            let input = std::fs::read_to_string(&input_path)
                .map_err(|e| OracleError::io("reading", &input_path, e))?;
            let context = crate::probe::ProbeContext {
                input_dir: dir,
                jsbsim_root: None,
            };
            crate::probe::run(&spec.probe, &input, &context).map(Record::Transcript)
        }
    }
}

/// `series` as its header and one line of `{:016x}` bits per row.
#[must_use]
pub fn bits_text(series: &Series) -> String {
    let mut out = series.columns.join(",");
    out.push('\n');
    for row in &series.rows {
        for (i, value) in row.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            // Writing to a String cannot fail.
            write!(out, "{:016x}", value.to_bits()).ok();
        }
        out.push('\n');
    }
    out
}

fn dump_blocking(repo_root: &Path, out: &Path, cases: &[String]) -> Result<(), OracleError> {
    std::fs::create_dir_all(out).map_err(|e| OracleError::io("creating", out, e))?;
    for case in cases {
        let record = record_blocking(repo_root, case)?;
        let path = out.join(record.file_name(case));
        std::fs::write(&path, record.text()).map_err(|e| OracleError::io("writing", &path, e))?;
        info!("{case}: recorded {} lines", record.text().lines().count());
    }
    Ok(())
}
