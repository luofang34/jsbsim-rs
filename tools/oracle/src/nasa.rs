// SPDX-License-Identifier: LGPL-2.1-or-later
//! Optional comparison of C++ JSBSim and the Rust port with the
//! atmospheric check-case data of NASA/TM-2015-218675. The data is not
//! vendored: `oracle/nasa/manifest.toml` pins each archive's URL, size and
//! sha256, `fetch` downloads them into `target/oracle/nasa`, and `compare`
//! refuses data that does not match the pins.

mod data;
mod envelope;

#[cfg(test)]
mod tests;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tracing::{error, info};

pub use data::{COLUMNS, Column, Simulator};
pub use envelope::{ColumnStats, compare_column};

use crate::manifest::ScriptSpec;
use crate::upstream::{self, Paths, run_blocking};
use crate::{OracleError, Series};

/// Samples per second compared; every simulator records at least this often.
const RATE_HZ: f64 = 10.0;

/// `oracle/nasa/manifest.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NasaManifest {
    /// Pinned archives.
    pub archive: Vec<Archive>,
    /// Check-cases run against them.
    pub case: Vec<Case>,
}

/// A pinned data archive.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    /// Name; the cache holds `<name>.zip`.
    pub name: String,
    /// Source URL.
    pub url: String,
    /// Size in bytes.
    pub size: u64,
    /// Lower-case hex SHA-256.
    pub sha256: String,
    /// Date the pins were taken.
    pub retrieved: String,
}

/// Where a case's script and aircraft live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Root {
    /// The JSBSim checkout for C++; its vendored subset for Rust.
    Jsbsim,
    /// `oracle/nasa` for both.
    Nasa,
}

/// A check-case.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Name, e.g. `Atmos_02`.
    pub name: String,
    /// Archive holding the simulators' recordings.
    pub archive: String,
    /// Where `script` and `aircraft_path` are.
    pub root: Root,
    /// Script path under the root.
    pub script: String,
    /// Aircraft directory under the root.
    pub aircraft_path: String,
    /// Where the TM defines the case.
    pub section: String,
}

impl NasaManifest {
    /// Reads `oracle/nasa/manifest.toml` of `repo_root`.
    pub fn read_blocking(repo_root: &Path) -> Result<Self, OracleError> {
        let path = repo_root.join("oracle").join("nasa").join("manifest.toml");
        let text =
            std::fs::read_to_string(&path).map_err(|e| OracleError::io("reading", &path, e))?;
        toml::from_str(&text).map_err(|source| OracleError::Toml {
            path,
            source: Box::new(source),
        })
    }

    fn archive(&self, name: &str) -> Result<&Archive, OracleError> {
        self.archive
            .iter()
            .find(|a| a.name == name)
            .ok_or_else(|| OracleError::NasaData {
                archive: name.to_owned(),
                message: "not in oracle/nasa/manifest.toml".to_owned(),
            })
    }
}

fn cache_dir(paths: &Paths) -> PathBuf {
    paths.work_dir().join("nasa")
}

/// Lower-case hex SHA-256 of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for b in Sha256::digest(bytes) {
        write!(hex, "{b:02x}").ok();
    }
    hex
}

/// Fails unless `bytes` are the archive the manifest pins.
pub fn verify(archive: &Archive, bytes: &[u8]) -> Result<(), OracleError> {
    let fail = |message: String| OracleError::NasaData {
        archive: archive.name.clone(),
        message,
    };
    if bytes.len() as u64 != archive.size {
        return Err(fail(format!(
            "{} bytes, the manifest pins {}; the data changed upstream",
            bytes.len(),
            archive.size
        )));
    }
    let actual = sha256_hex(bytes);
    if actual != archive.sha256 {
        return Err(fail(format!(
            "sha256 {actual}, the manifest pins {}; the data changed upstream",
            archive.sha256
        )));
    }
    Ok(())
}

/// Downloads, verifies and unpacks every pinned archive. An unreachable
/// site or a changed archive is an error.
pub fn fetch_blocking(paths: &Paths) -> Result<(), OracleError> {
    let manifest = NasaManifest::read_blocking(&paths.repo_root)?;
    let dir = cache_dir(paths);
    std::fs::create_dir_all(&dir).map_err(|e| OracleError::io("creating", &dir, e))?;
    for archive in &manifest.archive {
        let part = dir.join(format!("{}.zip.part", archive.name));
        run_blocking(
            Command::new("curl")
                .args(["-fsSL", "--max-time", "300", "-o"])
                .arg(&part)
                .arg(&archive.url),
        )?;
        let bytes = std::fs::read(&part).map_err(|e| OracleError::io("reading", &part, e))?;
        verify(archive, &bytes)?;
        let zip = dir.join(format!("{}.zip", archive.name));
        std::fs::rename(&part, &zip).map_err(|e| OracleError::io("renaming", &part, e))?;
        let out = dir.join(&archive.name);
        if out.exists() {
            std::fs::remove_dir_all(&out).map_err(|e| OracleError::io("removing", &out, e))?;
        }
        run_blocking(
            Command::new("unzip")
                .args(["-q", "-o"])
                .arg(&zip)
                .args(["-x", "__MACOSX/*", "-d"])
                .arg(&out),
        )?;
        info!(
            "{}: fetched and verified ({} bytes)",
            archive.name, archive.size
        );
    }
    Ok(())
}

/// The simulators of `archive`, after checking the cached archive against
/// its pin.
fn simulators_blocking(paths: &Paths, archive: &Archive) -> Result<Vec<Simulator>, OracleError> {
    let zip = cache_dir(paths).join(format!("{}.zip", archive.name));
    let bytes = std::fs::read(&zip).map_err(|e| OracleError::NasaData {
        archive: archive.name.clone(),
        message: format!("{}: {e}; run `oracle nasa fetch` first", zip.display()),
    })?;
    verify(archive, &bytes)?;
    data::read_simulators_blocking(&cache_dir(paths).join(&archive.name))
}

fn spec(case: &Case) -> ScriptSpec {
    let mut properties: Vec<String> = Vec::new();
    for c in COLUMNS {
        if !properties.iter().any(|p| p == c.property) {
            properties.push(c.property.to_owned());
        }
    }
    ScriptSpec {
        script: case.script.clone(),
        rate_hz: RATE_HZ,
        end_time: None,
        properties,
        aircraft_path: Some(case.aircraft_path.clone()),
        inputs: None,
    }
}

/// Logs how `candidate` compares with the simulators; returns whether every
/// variable lies within their spread.
fn report_side(case: &str, side: &str, simulators: &[Simulator], candidate: &Series) -> bool {
    let mut pass = true;
    for column in COLUMNS {
        let Some(s) = compare_column(column, simulators, candidate) else {
            continue;
        };
        let verdict = if s.passes() { "PASS" } else { "FAIL" };
        pass &= s.passes();
        info!(
            "{case} {side:<4} {:<33} sims {}, spread <= {:.3e}, outside <= {:.3e} at t={}, inside {}/{}, within spread {}/{} {verdict}",
            s.nasa,
            s.simulators,
            s.max_spread,
            s.max_excursion,
            s.worst_time,
            s.inside,
            s.samples,
            s.within_spread,
            s.samples
        );
    }
    pass
}

/// Logs the largest Rust-vs-C++ difference of each property.
fn report_parity(case: &str, cpp: &Series, rust: &Series) {
    for (i, property) in cpp.columns.iter().enumerate().skip(1) {
        let max = cpp
            .rows
            .iter()
            .zip(&rust.rows)
            .filter_map(|(a, b)| Some((a.get(i)? - b.get(i)?).abs()))
            .fold(0.0_f64, f64::max);
        info!("{case} parity {property:<33} max |Rust - C++| {max:.3e}");
    }
}

/// Runs `names` (all cases when empty) in C++ and Rust and compares both
/// with the NASA simulators' spread.
pub fn compare_blocking(paths: &Paths, names: &[String]) -> Result<ExitCode, OracleError> {
    let manifest = NasaManifest::read_blocking(&paths.repo_root)?;
    let nasa_root = paths.repo_root.join("oracle").join("nasa");
    let mut pass = true;
    for case in &manifest.case {
        if !names.is_empty() && !names.contains(&case.name) {
            continue;
        }
        info!("{}: TM-2015-218675 {}", case.name, case.section);
        let simulators = simulators_blocking(paths, manifest.archive(&case.archive)?)?;
        let spec = spec(case);
        let (cpp_root, rust_root) = match case.root {
            Root::Jsbsim => (
                paths.jsbsim_root.clone(),
                paths.repo_root.join("oracle").join("inputs"),
            ),
            Root::Nasa => (nasa_root.clone(), nasa_root.clone()),
        };
        let run_dir = paths
            .work_dir()
            .join("run")
            .join(format!("nasa-{}", case.name));
        let cpp = upstream::run_script_in_blocking(paths, &cpp_root, &spec, &run_dir)?.series;
        pass &= report_side(&case.name, "C++", &simulators, &cpp);
        let (rust, failure) = crate::port_script::run_script(&rust_root, &spec);
        if let Some(e) = failure {
            error!("{}: the Rust run failed: {e}", case.name);
            pass = false;
            continue;
        }
        pass &= report_side(&case.name, "Rust", &simulators, &rust);
        report_parity(&case.name, &cpp, &rust);
    }
    Ok(if pass {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
