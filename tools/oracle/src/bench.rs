// SPDX-License-Identifier: LGPL-2.1-or-later
//! `oracle bench`: frames per second of the Rust port against the C++
//! driver on the scripts and aircraft listed in `oracle/bench.toml`.
//!
//! Both sides follow one procedure per repeat: load the script, or the
//! aircraft with the [`AIRBORNE_IC`] properties set, into a fresh
//! simulation, `RunIC`, run the warm-up frames, then time the measured
//! frames with a monotonic clock. C++ reads the JSBSim checkout and Rust the
//! byte-identical vendored copies in `oracle/inputs`. Repeats alternate
//! between the sides so that machine load lands on both alike. The numbers
//! are for reports: timing is noisy, so no ratio fails the run, only a case
//! that cannot run.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::Deserialize;
use tracing::{error, info};

use crate::OracleError;
use crate::upstream::Paths;

mod cpp;
mod rust;
mod summary;

#[cfg(test)]
mod tests;

pub use cpp::parse_bench_output;
pub use summary::{Summary, fps, ratio};

/// The committed case list, relative to the repository root.
pub const CASES_FILE: &str = "oracle/bench.toml";

/// The initial condition an aircraft case flies from, set before `RunIC`:
/// airborne, so that every frame runs the aerodynamics and no contact
/// ends the run. `tests/zero_alloc.rs` flies its aircraft from it too.
pub const AIRBORNE_IC: [(&str, f64); 2] = [("ic/h-sl-ft", 10000.0), ("ic/u-fps", 500.0)];

/// One benchmarked script or aircraft.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchCase {
    /// Name used on the command line and in the report.
    pub name: String,
    /// Script path relative to the JSBSim root, e.g. `scripts/ball.xml`.
    /// A case names a script or an aircraft.
    #[serde(default)]
    pub script: Option<String>,
    /// Aircraft directory name, loaded with `LoadModel` and flown from
    /// [`AIRBORNE_IC`].
    #[serde(default)]
    pub aircraft: Option<String>,
    /// Aircraft directory relative to the JSBSim root; `aircraft` when
    /// absent.
    #[serde(default)]
    pub aircraft_path: Option<String>,
    /// Frames run after `RunIC` and before timing.
    pub warm_up: u32,
    /// Timed frames per repeat.
    pub frames: u32,
    /// Fresh simulations timed on each side.
    pub repeats: u32,
}

/// What a case runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Workload<'a> {
    /// A script path.
    Script(&'a str),
    /// An aircraft name.
    Aircraft(&'a str),
}

impl BenchCase {
    /// The script or aircraft; an error unless exactly one is given.
    pub fn workload(&self) -> Result<Workload<'_>, OracleError> {
        match (&self.script, &self.aircraft) {
            (Some(script), None) => Ok(Workload::Script(script)),
            (None, Some(aircraft)) => Ok(Workload::Aircraft(aircraft)),
            _ => Err(OracleError::Bench {
                case: self.name.clone(),
                message: "a case names exactly one of script and aircraft".to_owned(),
            }),
        }
    }
}

/// Contents of [`CASES_FILE`].
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchFile {
    /// Cases in report order.
    #[serde(rename = "case")]
    pub cases: Vec<BenchCase>,
}

impl BenchFile {
    /// Parses `text`, read from `path`.
    pub fn parse(text: &str, path: &Path) -> Result<Self, OracleError> {
        toml::from_str(text).map_err(|e| OracleError::Toml {
            path: path.to_path_buf(),
            source: Box::new(e),
        })
    }

    /// Reads and parses the file at `path`.
    pub fn read_blocking(path: &Path) -> Result<Self, OracleError> {
        let text =
            std::fs::read_to_string(path).map_err(|e| OracleError::io("reading", path, e))?;
        Self::parse(&text, path)
    }

    /// The cases called `names`, in that order; every case when empty.
    pub fn select(&self, names: &[String]) -> Result<Vec<BenchCase>, OracleError> {
        if names.is_empty() {
            return Ok(self.cases.clone());
        }
        names
            .iter()
            .map(|name| {
                self.cases
                    .iter()
                    .find(|case| &case.name == name)
                    .cloned()
                    .ok_or_else(|| OracleError::Bench {
                        case: name.clone(),
                        message: format!("not listed in {CASES_FILE}"),
                    })
            })
            .collect()
    }
}

/// Both sides' timings of one case, in ns per frame.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseResult {
    /// Case name.
    pub name: String,
    /// Timed frames per repeat.
    pub frames: u32,
    /// C++ driver.
    pub cpp: Summary,
    /// Rust port.
    pub rust: Summary,
}

/// Where the Rust port reads its inputs.
#[must_use]
pub fn rust_root(repo_root: &Path) -> PathBuf {
    repo_root.join("oracle").join("inputs")
}

/// Runs the selected cases and logs a table. A case that cannot run is
/// logged and makes the result [`ExitCode::FAILURE`]; the others still run.
pub fn run_blocking(paths: &Paths, names: &[String]) -> Result<ExitCode, OracleError> {
    let file = BenchFile::read_blocking(&paths.repo_root.join(CASES_FILE))?;
    let cases = file.select(names)?;
    let mut results = Vec::new();
    let mut code = ExitCode::SUCCESS;
    for case in &cases {
        match run_case_blocking(paths, case) {
            Ok(result) => {
                info!("{}", format_row(&result));
                results.push(result);
            }
            Err(e) => {
                error!("{e}");
                code = ExitCode::FAILURE;
            }
        }
    }
    info!("{}", format_header());
    for result in &results {
        info!("{}", format_row(result));
    }
    Ok(code)
}

fn run_case_blocking(paths: &Paths, case: &BenchCase) -> Result<CaseResult, OracleError> {
    let run_dir = paths.work_dir().join("bench").join(&case.name);
    std::fs::create_dir_all(&run_dir).map_err(|e| OracleError::io("creating", &run_dir, e))?;
    let rust_root = rust_root(&paths.repo_root);
    let mut cpp_ns = Vec::new();
    let mut rust_ns = Vec::new();
    for _ in 0..case.repeats {
        for elapsed in cpp::time_blocking(paths, case, &run_dir)? {
            cpp_ns.push(per_frame(elapsed, case.frames));
        }
        rust_ns.push(per_frame(rust::time(&rust_root, case)?, case.frames));
    }
    let summarize = |side: &str, samples: &[f64]| {
        Summary::of(samples).ok_or_else(|| OracleError::Bench {
            case: case.name.clone(),
            message: format!("no {side} timings"),
        })
    };
    Ok(CaseResult {
        name: case.name.clone(),
        frames: case.frames,
        cpp: summarize("C++", &cpp_ns)?,
        rust: summarize("Rust", &rust_ns)?,
    })
}

/// Nanoseconds per frame of a window of `frames` frames.
#[must_use]
pub fn per_frame(elapsed_ns: u64, frames: u32) -> f64 {
    elapsed_ns as f64 / f64::from(frames.max(1))
}

/// Column titles of [`format_row`].
#[must_use]
pub fn format_header() -> String {
    format!(
        "{:<18} {:>7}  {:>26}  {:>26}  {:>8}  {:>9}  {:>9}",
        "case",
        "frames",
        "C++ ns/frame med (min-max)",
        "Rust ns/frame med (min-max)",
        "Rust/C++",
        "Rust fps",
        "C++ fps"
    )
}

/// One report line.
#[must_use]
pub fn format_row(result: &CaseResult) -> String {
    let side = |s: &Summary| format!("{:.0} ({:.0}-{:.0})", s.median, s.min, s.max);
    format!(
        "{:<18} {:>7}  {:>26}  {:>26}  {:>8.3}  {:>9.0}  {:>9.0}",
        result.name,
        result.frames,
        side(&result.cpp),
        side(&result.rust),
        ratio(result.rust.median, result.cpp.median),
        fps(result.rust.median),
        fps(result.cpp.median)
    )
}
