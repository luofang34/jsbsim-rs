// SPDX-License-Identifier: LGPL-2.1-or-later
//! Command-line interface: `build`, `generate`, `check-drift`, `check-xml`,
//! `check-models`, `derive`, `port`, `compare` and `nasa`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use tracing::{error, info};

use crate::manifest;
use crate::upstream::{self, Paths};
use crate::{OracleError, Outcome, Series, ToleranceTable, compare};
use crate::{cases, corpus, nasa};

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
                .arg(jsbsim_root.clone())
                .arg(cases),
        )
        .subcommand(
            Command::new("check-xml")
                .about("Parse every JSBSim XML file in C++ and Rust and compare the trees")
                .arg(jsbsim_root.clone()),
        )
        .subcommand(
            Command::new("derive")
                .about("Regenerate the aircraft in oracle/derived from their upstream sources"),
        )
        .subcommand(
            Command::new("check-models")
                .about(
                    "Load every corpus aircraft in C++ and Rust and compare the covered properties",
                )
                .arg(jsbsim_root.clone()),
        )
        .subcommand(nasa_command(jsbsim_root))
        .subcommand(
            Command::new("port")
                .about("Run the Rust port on golden cases and compare them with their references")
                .arg(
                    Arg::new("case")
                        .action(ArgAction::Append)
                        .help("Golden case names; all cases when omitted"),
                ),
        )
        .subcommand(compare_command())
        .subcommand(
            Command::new("port-stats")
                .about("Run the Rust port on a script case and log its largest differences per property")
                .arg(Arg::new("case").required(true))
                .arg(
                    Arg::new("until")
                        .long("until")
                        .value_parser(value_parser!(f64))
                        .help("Compare rows up to this time, s"),
                ),
        )
}

fn nasa_command(jsbsim_root: Arg) -> Command {
    Command::new("nasa")
                .about("Optional comparison with the NASA TM-2015-218675 check-case data")
                .subcommand_required(true)
                .subcommand(
                    Command::new("fetch")
                        .about("Download the pinned archives into target/oracle/nasa and verify them"),
                )
                .subcommand(
                    Command::new("compare")
                        .about("Run the check-cases in C++ and Rust and compare both with the simulators' spread")
                        .arg(jsbsim_root)
                        .arg(
                            Arg::new("case")
                                .action(ArgAction::Append)
                                .help("Check-case names; all when omitted"),
                        ),
                )
}

fn compare_command() -> Command {
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
    if name == "port-stats" {
        let case = sub.get_one::<String>("case").cloned().unwrap_or_default();
        let until = sub.get_one::<f64>("until").copied();
        return crate::stats::port_stats_blocking(&repo_root, &case, until);
    }
    if name == "port" {
        let mut names: Vec<String> = sub
            .try_get_many::<String>("case")
            .ok()
            .flatten()
            .map(|values| values.cloned().collect())
            .unwrap_or_default();
        if names.is_empty() {
            names = manifest::list_cases_blocking(&repo_root.join("oracle").join("goldens"))?;
        }
        let mut code = ExitCode::SUCCESS;
        for case in &names {
            if cases::port_blocking(&repo_root, case)? != ExitCode::SUCCESS {
                code = ExitCode::FAILURE;
            }
        }
        return Ok(code);
    }
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
    if name == "derive" {
        return crate::derived::write_blocking(&repo_root).map(|()| ExitCode::SUCCESS);
    }
    if name == "nasa" {
        return run_nasa(repo_root, sub);
    }
    let paths = Paths {
        repo_root,
        jsbsim_root: path(sub, "jsbsim-root"),
    };
    upstream::build_blocking(&paths)?;
    match name {
        "build" => return Ok(ExitCode::SUCCESS),
        "check-xml" => return corpus::check_xml_blocking(&paths),
        "check-models" => return corpus::models::check_models_blocking(&paths),
        _ => {}
    }
    let cases = selected_cases(&paths, sub)?;
    match name {
        "generate" => cases::generate_blocking(&paths, &cases).map(|()| ExitCode::SUCCESS),
        "check-drift" => cases::check_drift_blocking(&paths, &cases),
        _ => Ok(ExitCode::SUCCESS),
    }
}

fn run_nasa(repo_root: PathBuf, sub: &ArgMatches) -> Result<ExitCode, OracleError> {
    let Some((name, sub)) = sub.subcommand() else {
        return Ok(ExitCode::FAILURE);
    };
    let jsbsim_root = sub
        .try_get_one::<PathBuf>("jsbsim-root")
        .ok()
        .flatten()
        .cloned()
        .unwrap_or_default();
    let paths = Paths {
        repo_root,
        jsbsim_root,
    };
    if name == "fetch" {
        return nasa::fetch_blocking(&paths).map(|()| ExitCode::SUCCESS);
    }
    upstream::build_blocking(&paths)?;
    let names: Vec<String> = sub
        .try_get_many::<String>("case")
        .ok()
        .flatten()
        .map(|values| values.cloned().collect())
        .unwrap_or_default();
    nasa::compare_blocking(&paths, &names)
}

fn selected_cases(paths: &Paths, sub: &ArgMatches) -> Result<Vec<String>, OracleError> {
    let named: Vec<String> = sub
        .try_get_many::<String>("case")
        .ok()
        .flatten()
        .map(|values| values.cloned().collect())
        .unwrap_or_default();
    if named.is_empty() {
        manifest::list_cases_blocking(&paths.goldens_dir())
    } else {
        Ok(named)
    }
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
pub(crate) fn report(case: &str, outcome: &Outcome) -> bool {
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
