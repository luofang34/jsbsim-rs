// SPDX-License-Identifier: LGPL-2.1-or-later
//! A scratch copy of oracle/nasa/aircraft/f16 under target/oracle/run, so
//! the F-16 checks can run the aircraft, or a variant of it, with their own
//! scripts and initial conditions in C++ JSBSim and in the port.

use std::path::{Path, PathBuf};

use crate::manifest::ScriptSpec;
use crate::upstream::{self, Paths};
use crate::{OracleError, Series};

/// Rows a check's run produced in C++ and in the port.
pub(super) struct Runs {
    /// C++ JSBSim.
    pub cpp: Series,
    /// The Rust port.
    pub rust: Series,
}

/// The committed F-16 directory.
fn f16_dir(paths: &Paths) -> PathBuf {
    paths.repo_root.join("oracle/nasa/aircraft/f16")
}

/// The committed f16.xml.
pub(super) fn f16_text_blocking(paths: &Paths) -> Result<String, OracleError> {
    let path = f16_dir(paths).join("f16.xml");
    std::fs::read_to_string(&path).map_err(|e| OracleError::io("reading", &path, e))
}

fn write(path: &Path, text: &str) -> Result<(), OracleError> {
    std::fs::write(path, text).map_err(|e| OracleError::io("writing", path, e))
}

/// A fresh root `target/oracle/run/nasa-<label>/root` whose aircraft/f16
/// holds `f16_xml` and the committed engine files, with an empty scripts
/// directory.
pub(super) fn create_blocking(
    paths: &Paths,
    label: &str,
    f16_xml: &str,
) -> Result<PathBuf, OracleError> {
    let root = paths
        .work_dir()
        .join("run")
        .join(format!("nasa-{label}"))
        .join("root");
    if root.exists() {
        std::fs::remove_dir_all(&root).map_err(|e| OracleError::io("removing", &root, e))?;
    }
    let aircraft = root.join("aircraft/f16");
    let engines = aircraft.join("Engines");
    for dir in [&engines, &root.join("scripts")] {
        std::fs::create_dir_all(dir).map_err(|e| OracleError::io("creating", dir, e))?;
    }
    write(&aircraft.join("f16.xml"), f16_xml)?;
    let source = f16_dir(paths).join("Engines");
    let entries = std::fs::read_dir(&source).map_err(|e| OracleError::io("listing", &source, e))?;
    for entry in entries {
        let from = entry
            .map_err(|e| OracleError::io("listing", &source, e))?
            .path();
        if let Some(name) = from.file_name() {
            std::fs::copy(&from, engines.join(name))
                .map_err(|e| OracleError::io("copying", &from, e))?;
        }
    }
    Ok(root)
}

/// Writes `text` as `<root>/<relative>`.
pub(super) fn add_file_blocking(
    root: &Path,
    relative: &str,
    text: &str,
) -> Result<(), OracleError> {
    write(&root.join(relative), text)
}

/// Runs `spec` under `root` in C++ (working directory `run_dir`) and in
/// the port; a port that stops early is an error.
pub(super) fn run_both_blocking(
    paths: &Paths,
    root: &Path,
    spec: &ScriptSpec,
    run_dir: &Path,
) -> Result<Runs, OracleError> {
    let cpp = upstream::run_script_in_blocking(paths, root, spec, run_dir)?.series;
    let (rust, failure) = crate::port_script::run_script(root, spec);
    if let Some(e) = failure {
        return Err(OracleError::NasaCheck {
            check: spec.script.clone(),
            message: format!("the Rust run stopped: {e}"),
        });
    }
    Ok(Runs { cpp, rust })
}

/// A spec sampling `properties` of `script` at `rate_hz`, until `end_time`.
pub(super) fn spec(
    script: &str,
    rate_hz: f64,
    end_time: Option<f64>,
    properties: &[&str],
) -> ScriptSpec {
    ScriptSpec {
        script: script.to_owned(),
        rate_hz,
        end_time,
        properties: properties.iter().map(|&p| p.to_owned()).collect(),
        aircraft_path: Some("aircraft".to_owned()),
        inputs: None,
        ground_callback: None,
    }
}

/// `property` in `series`' row `row` (0 is the row after the initial
/// conditions).
pub(super) fn value(series: &Series, row: usize, property: &str) -> Result<f64, OracleError> {
    let column = series.columns.iter().position(|c| c == property);
    column
        .and_then(|c| series.rows.get(row)?.get(c).copied())
        .ok_or_else(|| OracleError::NasaCheck {
            check: property.to_owned(),
            message: format!("no value in row {row}"),
        })
}
