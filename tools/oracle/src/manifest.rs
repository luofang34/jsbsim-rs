// SPDX-License-Identifier: LGPL-2.1-or-later
//! Golden case manifests.
//!
//! Each case lives in `oracle/goldens/<case>/`: `manifest.toml` states what
//! to run (`[case]`) and, once generated, where the reference came from
//! (`[provenance]`); `reference.csv` holds the C++ output.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::OracleError;

#[cfg(test)]
mod tests;

/// Name of the manifest file inside a case directory.
pub const MANIFEST_FILE: &str = "manifest.toml";
/// Name of the reference CSV inside a case directory.
pub const REFERENCE_FILE: &str = "reference.csv";
/// Name of the reference transcript inside a case directory.
pub const TRANSCRIPT_FILE: &str = "reference.txt";
/// Name of the probe input inside a case directory.
pub const INPUT_FILE: &str = "input.txt";

/// What to run for a case, by kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CaseSpec {
    /// A JSBSim script sampled into a time series.
    Script(ScriptSpec),
    /// A probe whose transcript must match the reference line for line.
    Transcript(TranscriptSpec),
}

/// A JSBSim script sampled at a fixed rate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptSpec {
    /// Script path relative to the JSBSim root, e.g. `scripts/ball.xml`.
    pub script: String,
    /// Sampling rate, converted to a frame interval as JSBSim's
    /// `FGOutputType::SetRateHz` does.
    pub rate_hz: f64,
    /// Stop after this simulation time; the script's own end when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<f64>,
    /// Property paths to sample, in column order.
    pub properties: Vec<String>,
    /// Aircraft directory relative to the JSBSim root; `aircraft` when
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aircraft_path: Option<String>,
}

/// A driver probe fed with `input.txt` from the case directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptSpec {
    /// Driver mode, e.g. `properties`.
    pub probe: String,
}

/// Where a reference came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// `git rev-parse HEAD` of the JSBSim checkout, which had no tracked
    /// changes.
    pub upstream_commit: String,
    /// Driver sources that produced the reference, relative to the
    /// repository root.
    pub driver: String,
    /// Extra compiler flags libJSBSim and the driver were built with.
    pub cxx_flags: String,
    /// Integration time step reported by the script.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dt: Option<f64>,
    /// Frames between samples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every_n_frames: Option<u64>,
    /// Data rows (CSV) or lines (transcript) in the reference.
    pub rows: usize,
}

/// Contents of `manifest.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// What to run.
    pub case: CaseSpec,
    /// Absent until the reference has been generated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

impl Manifest {
    /// Parses manifest TOML. `path` labels errors.
    pub fn parse(text: &str, path: &Path) -> Result<Self, OracleError> {
        toml::from_str(text).map_err(|e| OracleError::Toml {
            path: path.to_owned(),
            source: Box::new(e),
        })
    }

    /// Reads the manifest of the case in `case_dir`.
    pub fn read_blocking(case_dir: &Path) -> Result<Self, OracleError> {
        let path = case_dir.join(MANIFEST_FILE);
        let text =
            std::fs::read_to_string(&path).map_err(|e| OracleError::io("reading", &path, e))?;
        Self::parse(&text, &path)
    }

    /// Writes the manifest of the case in `case_dir`.
    pub fn write_blocking(&self, case_dir: &Path) -> Result<(), OracleError> {
        let path = case_dir.join(MANIFEST_FILE);
        let text = toml::to_string(self).map_err(|source| OracleError::TomlWrite {
            path: path.clone(),
            source,
        })?;
        std::fs::write(&path, text).map_err(|e| OracleError::io("writing", &path, e))
    }

    /// Column header a reference for this case must have, for cases that
    /// produce a series.
    #[must_use]
    pub fn expected_columns(&self) -> Option<Vec<String>> {
        match &self.case {
            CaseSpec::Script(spec) => Some(
                std::iter::once(crate::series::TIME_COLUMN.to_owned())
                    .chain(spec.properties.iter().cloned())
                    .collect(),
            ),
            CaseSpec::Transcript(_) => None,
        }
    }
}

/// Names of the case directories under `goldens_dir`, sorted.
pub fn list_cases_blocking(goldens_dir: &Path) -> Result<Vec<String>, OracleError> {
    let entries =
        std::fs::read_dir(goldens_dir).map_err(|e| OracleError::io("listing", goldens_dir, e))?;
    let mut cases = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| OracleError::io("listing", goldens_dir, e))?;
        if entry.path().join(MANIFEST_FILE).is_file() {
            cases.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    cases.sort();
    Ok(cases)
}

/// Directory of `case` under `goldens_dir`.
#[must_use]
pub fn case_dir(goldens_dir: &Path, case: &str) -> PathBuf {
    goldens_dir.join(case)
}
