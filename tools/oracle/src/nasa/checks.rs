// SPDX-License-Identifier: LGPL-2.1-or-later
//! The F-16 checks beside the envelope comparison, each a subcommand of
//! `oracle nasa`: the model's mass properties, the DAVE-ML static shots,
//! the control law frame by frame and the integration step's stability.

use std::process::ExitCode;

use tracing::{error, info};

use super::{control_law, limit_cycle, mass, shots};
use crate::OracleError;
use crate::upstream::Paths;

/// The checks, by subcommand name, with what each verifies.
pub const CHECKS: [(&str, &str); 4] = [
    (
        "mass",
        "Compare f16.xml's mass properties in C++ and Rust with F16_inertia.dml",
    ),
    (
        "shots",
        "Run F16_aero.dml's and F16_prop.dml's static shots through f16.xml in C++ and Rust",
    ),
    (
        "control-law",
        "Check f16.xml's control law frame by frame against F16_control.dml and F16_gnc.dml",
    ),
    (
        "limit-cycle",
        "Require check-case 13.2 free of a roll limit cycle in C++ and Rust",
    ),
];

fn verdict(check: &str, failures: usize, detail: &str) -> ExitCode {
    if failures == 0 {
        info!("nasa {check}: ok ({detail})");
        ExitCode::SUCCESS
    } else {
        error!("nasa {check}: {failures} failed ({detail})");
        ExitCode::FAILURE
    }
}

/// Runs check `name` of [`CHECKS`].
pub fn run_blocking(paths: &Paths, name: &str) -> Result<ExitCode, OracleError> {
    Ok(match name {
        "mass" => verdict(name, mass::check_blocking(paths)?, "C++ and Rust"),
        "shots" => {
            let t = shots::check_blocking(paths)?;
            let detail = format!(
                "{} shots, {} comparisons in C++ and Rust",
                t.shots, t.compared
            );
            verdict(name, t.failed, &detail)
        }
        "control-law" => verdict(
            name,
            control_law::check_blocking(paths)?,
            &control_law::CASES.join(", "),
        ),
        "limit-cycle" => verdict(name, limit_cycle::check_blocking(paths)?, "C++ and Rust"),
        other => {
            return Err(OracleError::NasaCheck {
                check: other.to_owned(),
                message: "no such check".to_owned(),
            });
        }
    })
}
