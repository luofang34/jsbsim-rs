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
    /// Variables JSBSim itself misses the simulators on.
    #[serde(default)]
    pub expected_miss: Vec<ExpectedMiss>,
    /// A looser Rust-vs-C++ bound than [`PARITY_REL`], when the case needs one.
    #[serde(default)]
    pub parity: Option<Parity>,
}

/// The Rust-vs-C++ bound of one case and why it is looser than
/// [`PARITY_REL`].
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parity {
    /// Largest difference allowed, relative as for [`PARITY_REL`].
    pub rel: f64,
    /// What makes Rust and C++ differ by more than [`PARITY_REL`].
    pub cause: String,
}

impl Case {
    /// Largest Rust-vs-C++ difference allowed in this case, relative to the
    /// largest magnitude of the property over the run (or 1 when smaller).
    #[must_use]
    pub fn parity_rel(&self) -> f64 {
        self.parity.as_ref().map_or(PARITY_REL, |p| p.rel)
    }
}

/// A variable C++ JSBSim lies outside the simulators on, and why. The
/// comparison fails if it stops missing, so the list stays current.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedMiss {
    /// TM variable, e.g. `gePosition_ft_X`.
    pub variable: String,
    /// What makes JSBSim miss.
    pub cause: String,
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

/// What a case samples: every property of [`COLUMNS`] at [`RATE_HZ`].
pub(crate) fn spec(case: &Case) -> ScriptSpec {
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
        ground_callback: None,
    }
}

/// Where C++ and Rust find `case`'s script and aircraft.
pub(crate) fn roots(paths: &Paths, case: &Case) -> (PathBuf, PathBuf) {
    match case.root {
        Root::Jsbsim => (
            paths.jsbsim_root.clone(),
            paths.repo_root.join("oracle").join("inputs"),
        ),
        Root::Nasa => {
            let nasa = paths.repo_root.join("oracle").join("nasa");
            (nasa.clone(), nasa)
        }
    }
}

/// Largest Rust-vs-C++ difference allowed, relative to the largest
/// magnitude of the property over the run (or 1 when smaller), unless the
/// case states a looser [`Parity`].
pub const PARITY_REL: f64 = 1e-12;

/// Logs each variable's verdict: C++ inside the simulators at every sample,
/// or missing where the manifest names the cause; Rust outside on exactly
/// the same samples. Returns whether every verdict holds.
fn report_case(case: &Case, simulators: &[Simulator], cpp: &Series, rust: &Series) -> bool {
    let mut pass = true;
    for column in COLUMNS {
        let (Some(c), Some(r)) = (
            compare_column(column, simulators, cpp),
            compare_column(column, simulators, rust),
        ) else {
            continue;
        };
        let expected = case
            .expected_miss
            .iter()
            .find(|m| m.variable == column.nasa);
        let verdict = match (c.passes(), expected) {
            (true, None) => "PASS".to_owned(),
            (false, Some(m)) => format!("EXPECTED MISS ({})", m.cause),
            (true, Some(_)) => {
                pass = false;
                "FAIL: listed as an expected miss but inside; update oracle/nasa/manifest.toml"
                    .to_owned()
            }
            (false, None) => {
                pass = false;
                "FAIL".to_owned()
            }
        };
        let same = c.outside == r.outside;
        pass &= same;
        info!(
            "{} {:<33} sims {}, spread <= {:.3e}, C++ outside <= {:.3e} at t={}, inside {}/{}; Rust inside {}/{}, {}: {verdict}",
            case.name,
            c.nasa,
            c.simulators,
            c.max_spread,
            c.max_excursion,
            c.worst_time,
            c.inside,
            c.samples,
            r.inside,
            r.samples,
            if same {
                "same samples"
            } else {
                "DIFFERENT samples (FAIL)"
            },
        );
    }
    pass
}

/// Logs the largest Rust-vs-C++ difference of each property; returns
/// whether all are within `rel` of their magnitude.
fn report_parity(case: &str, rel: f64, cpp: &Series, rust: &Series) -> bool {
    let mut pass = cpp.rows.len() == rust.rows.len();
    for (i, property) in cpp.columns.iter().enumerate().skip(1) {
        let (mut max, mut magnitude) = (0.0_f64, 1.0_f64);
        for (a, b) in cpp.rows.iter().zip(&rust.rows) {
            if let (Some(a), Some(b)) = (a.get(i), b.get(i)) {
                max = max.max((a - b).abs());
                magnitude = magnitude.max(a.abs());
            }
        }
        let ok = max <= rel * magnitude;
        pass &= ok;
        info!(
            "{case} parity {property:<33} max |Rust - C++| {max:.3e}, {:.3e} of its magnitude{}",
            max / magnitude,
            if ok { "" } else { " FAIL" }
        );
    }
    pass
}

/// Runs `names` (all cases when empty) in C++ and Rust and compares both
/// with the NASA simulators' spread.
pub fn compare_blocking(paths: &Paths, names: &[String]) -> Result<ExitCode, OracleError> {
    let manifest = NasaManifest::read_blocking(&paths.repo_root)?;
    let mut pass = true;
    for case in &manifest.case {
        if !names.is_empty() && !names.contains(&case.name) {
            continue;
        }
        info!("{}: TM-2015-218675 {}", case.name, case.section);
        let simulators = simulators_blocking(paths, manifest.archive(&case.archive)?)?;
        let spec = spec(case);
        let (cpp_root, rust_root) = roots(paths, case);
        let run_dir = paths
            .work_dir()
            .join("run")
            .join(format!("nasa-{}", case.name));
        let cpp = upstream::run_script_in_blocking(paths, &cpp_root, &spec, &run_dir)?.series;
        let (rust, failure) = crate::port_script::run_script(&rust_root, &spec);
        if let Some(e) = failure {
            error!("{}: the Rust run failed: {e}", case.name);
            pass = false;
            continue;
        }
        pass &= report_case(case, &simulators, &cpp, &rust);
        if let Some(p) = &case.parity {
            info!("{}: parity within {:e} ({})", case.name, p.rel, p.cause);
        }
        pass &= report_parity(&case.name, case.parity_rel(), &cpp, &rust);
    }
    Ok(if pass {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
