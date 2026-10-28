// SPDX-License-Identifier: LGPL-2.1-or-later
//! Generating golden cases from the C++ reference and checking them for
//! drift.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use tracing::{error, info};

use crate::cli::report;
use crate::manifest::{
    self, CaseSpec, INPUT_FILE, Manifest, Provenance, REFERENCE_FILE, TRANSCRIPT_FILE,
};
use crate::transcript::{TranscriptOutcome, compare_transcript};
use crate::upstream::{self, Paths};
use crate::{OracleError, Series, ToleranceTable, compare};

/// Freshly produced reference output of a case.
#[derive(Debug, Clone)]
pub enum Fresh {
    /// Sampled time series.
    Series(Series),
    /// Probe transcript.
    Transcript(String),
}

fn run_dir(paths: &Paths, case: &str) -> PathBuf {
    paths.work_dir().join("run").join(case)
}

/// Runs `case` through the C++ driver.
pub fn run_fresh_blocking(
    paths: &Paths,
    case: &str,
    manifest: &Manifest,
) -> Result<(Fresh, Provenance), OracleError> {
    let dir = manifest::case_dir(&paths.goldens_dir(), case);
    match &manifest.case {
        CaseSpec::Script(spec) => {
            let run = upstream::run_script_blocking(paths, spec, &run_dir(paths, case))?;
            Ok((Fresh::Series(run.series), run.provenance))
        }
        CaseSpec::Transcript(spec) => {
            let run = upstream::run_transcript_blocking(
                paths,
                spec,
                &dir.join(INPUT_FILE),
                &run_dir(paths, case),
            )?;
            Ok((Fresh::Transcript(run.text), run.provenance))
        }
    }
}

/// Regenerates the references and provenance of `cases`.
pub fn generate_blocking(paths: &Paths, cases: &[String]) -> Result<(), OracleError> {
    for case in cases {
        let dir = manifest::case_dir(&paths.goldens_dir(), case);
        let mut manifest = Manifest::read_blocking(&dir)?;
        let (fresh, provenance) = run_fresh_blocking(paths, case, &manifest)?;
        match &fresh {
            Fresh::Series(series) => series.write_blocking(&dir.join(REFERENCE_FILE))?,
            Fresh::Transcript(text) => write_blocking(&dir.join(TRANSCRIPT_FILE), text)?,
        }
        info!(
            "{case}: {} rows at {}",
            provenance.rows, provenance.upstream_commit
        );
        manifest.provenance = Some(provenance);
        manifest.write_blocking(&dir)?;
    }
    Ok(())
}

fn write_blocking(path: &Path, text: &str) -> Result<(), OracleError> {
    std::fs::write(path, text).map_err(|e| OracleError::io("writing", path, e))
}

/// Regenerates `cases` and compares them with the committed references.
pub fn check_drift_blocking(paths: &Paths, cases: &[String]) -> Result<ExitCode, OracleError> {
    let table = ToleranceTable::read_blocking(&paths.tolerances_file())?;
    let mut code = ExitCode::SUCCESS;
    for case in cases {
        let dir = manifest::case_dir(&paths.goldens_dir(), case);
        let manifest = Manifest::read_blocking(&dir)?;
        let recorded = manifest.provenance.as_ref().ok_or_else(|| {
            OracleError::Provenance(format!("{case}: manifest has no provenance; run generate"))
        })?;
        let (fresh, provenance) = run_fresh_blocking(paths, case, &manifest)?;
        if provenance.cxx_flags != recorded.cxx_flags {
            error!(
                "{case}: reference was built with {:?} but the oracle uses {:?}",
                recorded.cxx_flags, provenance.cxx_flags
            );
            code = ExitCode::FAILURE;
        }
        if provenance.upstream_commit != recorded.upstream_commit {
            error!(
                "{case}: reference is from {} but the checkout is at {}",
                recorded.upstream_commit, provenance.upstream_commit
            );
            code = ExitCode::FAILURE;
        }
        let agrees = match fresh {
            Fresh::Series(series) => {
                let reference = Series::read_blocking(&dir.join(REFERENCE_FILE))?;
                report(case, &compare(case, &reference, &series, &table)?)
            }
            Fresh::Transcript(text) => {
                let path = dir.join(TRANSCRIPT_FILE);
                let reference = std::fs::read_to_string(&path)
                    .map_err(|e| OracleError::io("reading", &path, e))?;
                report_transcript(case, &compare_transcript(case, &reference, &text, &table)?)
            }
        };
        if !agrees {
            code = ExitCode::FAILURE;
        }
    }
    Ok(code)
}

/// Logs a transcript comparison; returns whether it agreed.
pub fn report_transcript(case: &str, outcome: &TranscriptOutcome) -> bool {
    match &outcome.divergence {
        None => {
            info!(
                "{case}: PASS, {} lines; {} numbers differ, max |diff| {:e}, max rel {:e}",
                outcome.lines, outcome.numbers_differing, outcome.max_abs, outcome.max_rel
            );
            true
        }
        Some(divergence) => {
            error!("{case}: FIRST DIVERGENCE {divergence}");
            false
        }
    }
}

/// Runs the Rust twin of transcript case `case`, writes its output under
/// `target/oracle/port/` and compares it with the reference.
pub fn port_blocking(repo_root: &Path, case: &str) -> Result<ExitCode, OracleError> {
    let dir = repo_root.join("oracle").join("goldens").join(case);
    let manifest = Manifest::read_blocking(&dir)?;
    let CaseSpec::Transcript(spec) = &manifest.case else {
        return Err(OracleError::Provenance(format!(
            "{case} is not a transcript case"
        )));
    };
    let input_path = dir.join(INPUT_FILE);
    let input = std::fs::read_to_string(&input_path)
        .map_err(|e| OracleError::io("reading", &input_path, e))?;
    let context = crate::probe::ProbeContext {
        input_dir: dir.clone(),
        jsbsim_root: None,
    };
    let candidate = crate::probe::run(&spec.probe, &input, &context)?;
    let out_dir = repo_root.join("target").join("oracle").join("port");
    std::fs::create_dir_all(&out_dir).map_err(|e| OracleError::io("creating", &out_dir, e))?;
    write_blocking(&out_dir.join(format!("{case}.txt")), &candidate)?;
    let reference_path = dir.join(TRANSCRIPT_FILE);
    let reference = std::fs::read_to_string(&reference_path)
        .map_err(|e| OracleError::io("reading", &reference_path, e))?;
    let table = ToleranceTable::read_blocking(&repo_root.join("oracle").join("tolerances.toml"))?;
    let outcome = compare_transcript(case, &reference, &candidate, &table)?;
    Ok(if report_transcript(case, &outcome) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
