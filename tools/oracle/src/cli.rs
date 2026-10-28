// SPDX-License-Identifier: LGPL-2.1-or-later
//! Command-line interface: `build`, `generate`, `check-drift` and `compare`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use tracing::{error, info};

use crate::manifest::{self, Manifest, REFERENCE_FILE};
use crate::upstream::{self, Paths};
use crate::{OracleError, Outcome, Series, ToleranceTable, compare};

#[cfg(test)]
mod tests;

/// Builds the argument parser.
#[must_use]
pub fn command() -> Command {
    let repo_root = Arg::new("repo-root")
        .long("repo-root")
        .value_parser(value_parser!(PathBuf))
        .default_value(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .global(true)
        .help("Root of the jsbsim-rs repository");
    let jsbsim_root = Arg::new("jsbsim-root")
        .long("jsbsim-root")
        .env("JSBSIM_ROOT")
        .value_parser(value_parser!(PathBuf))
        .required(true)
        .help("Read-only C++ JSBSim checkout");
    let cases = Arg::new("case")
        .action(ArgAction::Append)
        .help("Golden case names; all cases when omitted");
    Command::new("oracle")
        .about("C++ JSBSim reference oracle")
        .subcommand_required(true)
        .arg(repo_root)
        .subcommand(
            Command::new("build")
                .about("Build C++ JSBSim out of tree and compile the driver")
                .arg(jsbsim_root.clone()),
        )
        .subcommand(
            Command::new("generate")
                .about("Run cases and write their reference CSVs and provenance")
                .arg(jsbsim_root.clone())
                .arg(cases.clone()),
        )
        .subcommand(
            Command::new("check-drift")
                .about("Regenerate cases and compare them against the committed references")
                .arg(jsbsim_root)
                .arg(cases),
        )
        .subcommand(
            Command::new("compare")
                .about("Compare a candidate CSV against a reference CSV")
                .arg(Arg::new("case").long("case").required(true))
                .arg(path_arg("reference"))
                .arg(path_arg("candidate"))
                .arg(
                    Arg::new("tolerances")
                        .long("tolerances")
                        .value_parser(value_parser!(PathBuf))
                        .help("Tolerance table; oracle/tolerances.toml by default"),
                ),
        )
}

fn path_arg(name: &'static str) -> Arg {
    Arg::new(name)
        .required(true)
        .value_parser(value_parser!(PathBuf))
}

fn path(matches: &ArgMatches, name: &str) -> PathBuf {
    matches
        .get_one::<PathBuf>(name)
        .cloned()
        .unwrap_or_default()
}

/// Parses `args` and runs the selected subcommand. A divergence yields
/// [`ExitCode::FAILURE`]; operational problems yield an error.
pub fn run<I, T>(args: I) -> Result<ExitCode, OracleError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let matches = command().try_get_matches_from(args)?;
    let repo_root = path(&matches, "repo-root");
    let Some((name, sub)) = matches.subcommand() else {
        return Ok(ExitCode::FAILURE);
    };
    if name == "compare" {
        let tolerances = sub
            .get_one::<PathBuf>("tolerances")
            .cloned()
            .unwrap_or_else(|| repo_root.join("oracle").join("tolerances.toml"));
        let case = sub.get_one::<String>("case").cloned().unwrap_or_default();
        return compare_files_blocking(
            &case,
            &path(sub, "reference"),
            &path(sub, "candidate"),
            &tolerances,
        );
    }
    let paths = Paths {
        repo_root,
        jsbsim_root: path(sub, "jsbsim-root"),
    };
    upstream::build_blocking(&paths)?;
    let cases = selected_cases(&paths, sub)?;
    match name {
        "generate" => generate_blocking(&paths, &cases).map(|()| ExitCode::SUCCESS),
        "check-drift" => check_drift_blocking(&paths, &cases),
        _ => Ok(ExitCode::SUCCESS),
    }
}

fn selected_cases(paths: &Paths, sub: &ArgMatches) -> Result<Vec<String>, OracleError> {
    let named: Vec<String> = sub
        .get_many::<String>("case")
        .map(|values| values.cloned().collect())
        .unwrap_or_default();
    if named.is_empty() {
        manifest::list_cases_blocking(&paths.goldens_dir())
    } else {
        Ok(named)
    }
}

fn generate_blocking(paths: &Paths, cases: &[String]) -> Result<(), OracleError> {
    for case in cases {
        let dir = manifest::case_dir(&paths.goldens_dir(), case);
        let mut manifest = Manifest::read_blocking(&dir)?;
        let run = upstream::run_case_blocking(paths, &manifest.case, &run_dir(paths, case))?;
        run.series.write_blocking(&dir.join(REFERENCE_FILE))?;
        info!(
            "{case}: {} rows x {} properties at {}",
            run.provenance.rows,
            manifest.case.properties.len(),
            run.provenance.upstream_commit
        );
        manifest.provenance = Some(run.provenance);
        manifest.write_blocking(&dir)?;
    }
    Ok(())
}

fn check_drift_blocking(paths: &Paths, cases: &[String]) -> Result<ExitCode, OracleError> {
    let table = ToleranceTable::read_blocking(&paths.tolerances_file())?;
    let mut code = ExitCode::SUCCESS;
    for case in cases {
        let dir = manifest::case_dir(&paths.goldens_dir(), case);
        let manifest = Manifest::read_blocking(&dir)?;
        let recorded = manifest.provenance.as_ref().ok_or_else(|| {
            OracleError::Provenance(format!("{case}: manifest has no provenance; run generate"))
        })?;
        let run = upstream::run_case_blocking(paths, &manifest.case, &run_dir(paths, case))?;
        if run.provenance.upstream_commit != recorded.upstream_commit {
            error!(
                "{case}: reference is from {} but the checkout is at {}",
                recorded.upstream_commit, run.provenance.upstream_commit
            );
            code = ExitCode::FAILURE;
        }
        let reference = Series::read_blocking(&dir.join(REFERENCE_FILE))?;
        let outcome = compare(case, &reference, &run.series, &table)?;
        if !report(case, &outcome) {
            code = ExitCode::FAILURE;
        }
    }
    Ok(code)
}

fn compare_files_blocking(
    case: &str,
    reference: &Path,
    candidate: &Path,
    tolerances: &Path,
) -> Result<ExitCode, OracleError> {
    let table = ToleranceTable::read_blocking(tolerances)?;
    let outcome = compare(
        case,
        &Series::read_blocking(reference)?,
        &Series::read_blocking(candidate)?,
        &table,
    )?;
    Ok(if report(case, &outcome) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Logs the outcome; returns whether the candidate agreed.
fn report(case: &str, outcome: &Outcome) -> bool {
    match &outcome.divergence {
        None => {
            info!(
                "{case}: PASS, {} rows x {} columns within tolerance",
                outcome.rows_compared, outcome.columns
            );
            true
        }
        Some(divergence) => {
            error!("{case}: FIRST DIVERGENCE {divergence}");
            false
        }
    }
}

fn run_dir(paths: &Paths, case: &str) -> PathBuf {
    paths.work_dir().join("run").join(case)
}
