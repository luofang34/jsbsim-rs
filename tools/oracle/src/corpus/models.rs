// SPDX-License-Identifier: LGPL-2.1-or-later
//! Whole-aircraft loading: every corpus aircraft loads in C++ and Rust and
//! leaves the same properties in the subtrees the port covers, or the Rust
//! load stops with a typed Unsupported naming the construct it needs. What
//! each aircraft does is pinned in `oracle/model-loading.txt`, so an
//! aircraft that starts to fail differently is caught.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use tracing::{error, info};

use crate::OracleError;
use crate::manifest::TranscriptSpec;
use crate::probe::{self, ProbeContext};
use crate::upstream::{self, Paths};

/// Relative difference allowed between C++ and Rust values: the last digit
/// of values derived through `pow`/`exp`/`sin` (see oracle/tolerances.toml).
const REL_TOLERANCE: f64 = 4e-15;

/// The result of one aircraft on one side.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// `ok`, `load-error` or `unsupported <construct> (<where>)`.
    pub status: String,
    /// Property path to printed value.
    pub values: BTreeMap<String, String>,
}

/// Splits a probe transcript into aircraft name and result.
#[must_use]
pub fn parse(transcript: &str) -> Vec<(String, Block)> {
    let mut blocks: Vec<(String, Block)> = Vec::new();
    for line in transcript.lines() {
        if let Some(entry) = line.strip_prefix("  ") {
            if let (Some((_, block)), Some((k, v))) = (blocks.last_mut(), entry.split_once(" = ")) {
                block.values.insert(k.to_owned(), v.to_owned());
            }
            continue;
        }
        let Some((head, status)) = line.split_once(" =>") else {
            continue;
        };
        let name = head.strip_prefix("model ").unwrap_or(head).to_owned();
        blocks.push((
            name,
            Block {
                status: status.trim().to_owned(),
                values: BTreeMap::new(),
            },
        ));
    }
    blocks
}

fn values_agree(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => (x - y).abs() <= REL_TOLERANCE * x.abs().max(y.abs()),
        _ => false,
    }
}

/// Differences between the C++ and Rust properties, one line each.
#[must_use]
pub fn differences(cpp: &Block, rust: &Block) -> Vec<String> {
    let mut out = Vec::new();
    for (k, v) in &cpp.values {
        match rust.values.get(k) {
            None => out.push(format!("{k}: C++ {v}, missing in Rust")),
            Some(r) if !values_agree(v, r) => out.push(format!("{k}: C++ {v}, Rust {r}")),
            Some(_) => {}
        }
    }
    for (k, v) in &rust.values {
        if !cpp.values.contains_key(k) {
            out.push(format!("{k}: Rust {v}, missing in C++"));
        }
    }
    out
}

/// What the Rust side of an aircraft must do: `ok`, `load-error`, or the
/// Unsupported status line.
fn outcome(cpp: &Block, rust: &Block) -> Result<String, Vec<String>> {
    if rust.status.starts_with("unsupported ") {
        return Ok(rust.status.clone());
    }
    if cpp.status != rust.status {
        return Err(vec![format!("C++ {}, Rust {}", cpp.status, rust.status)]);
    }
    let diffs = differences(cpp, rust);
    if diffs.is_empty() {
        Ok(rust.status.clone())
    } else {
        Err(diffs)
    }
}

/// Every `aircraft/<name>/<name>.xml` of the JSBSim tree, sorted.
fn aircraft_blocking(root: &Path) -> Result<Vec<String>, OracleError> {
    let dir = root.join("aircraft");
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| OracleError::io("listing", &dir, e))? {
        let path = entry
            .map_err(|e| OracleError::io("listing", &dir, e))?
            .path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if path.join(format!("{name}.xml")).is_file() {
            names.push(name.to_owned());
        }
    }
    names.sort();
    Ok(names)
}

fn read_expected_blocking(path: &Path) -> Result<BTreeMap<String, String>, OracleError> {
    let text = std::fs::read_to_string(path).map_err(|e| OracleError::io("reading", path, e))?;
    Ok(text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once(' '))
        .map(|(n, s)| (n.to_owned(), s.to_owned()))
        .collect())
}

/// Runs the check; failure when an aircraft disagrees or departs from
/// `oracle/model-loading.txt`.
pub fn check_models_blocking(paths: &Paths) -> Result<ExitCode, OracleError> {
    let names = aircraft_blocking(&paths.jsbsim_root)?;
    let run_dir = paths.work_dir().join("run").join("models");
    std::fs::create_dir_all(&run_dir).map_err(|e| OracleError::io("creating", &run_dir, e))?;
    let input_path = run_dir.join("input.txt");
    let input: String = names.iter().map(|n| format!("model {n}\n")).collect();
    std::fs::write(&input_path, &input).map_err(|e| OracleError::io("writing", &input_path, e))?;
    let spec = TranscriptSpec {
        probe: "models".to_owned(),
        unsupported: Vec::new(),
    };
    let cpp = upstream::run_transcript_blocking(paths, &spec, &input_path, &run_dir)?;
    let context = ProbeContext {
        input_dir: run_dir.clone(),
        jsbsim_root: Some(paths.jsbsim_root.clone()),
    };
    let rust = probe::run("models", &input, &context)?;
    let expected_path = paths.repo_root.join("oracle").join("model-loading.txt");
    let expected = read_expected_blocking(&expected_path)?;
    let (cpp, rust) = (parse(&cpp.text), parse(&rust));
    let mut ok = cpp.len() == rust.len() && cpp.len() == names.len();
    let mut actual = String::new();
    for ((name, c), (_, r)) in cpp.iter().zip(&rust) {
        match outcome(c, r) {
            Ok(status) => {
                if status == "ok" {
                    info!("models: {name}: PASS, {} properties", c.values.len());
                }
                if expected.get(name) != Some(&status) {
                    error!(
                        "models: {name}: {status}; oracle/model-loading.txt expects {:?}",
                        expected.get(name)
                    );
                    ok = false;
                }
                actual.push_str(&format!("{name} {status}\n"));
            }
            Err(diffs) => {
                ok = false;
                error!("models: {name}: FAIL, {} differences", diffs.len());
                for d in diffs.iter().take(20) {
                    error!("models: {name}:   {d}");
                }
            }
        }
    }
    let loaded = actual.lines().filter(|l| l.ends_with(" ok")).count();
    info!(
        "models: {} aircraft, {loaded} load in Rust, the rest Unsupported",
        names.len()
    );
    if !ok {
        let path = run_dir.join("model-loading.txt");
        std::fs::write(&path, &actual).map_err(|e| OracleError::io("writing", &path, e))?;
        error!("models: the outcomes seen are in {}", path.display());
    }
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
