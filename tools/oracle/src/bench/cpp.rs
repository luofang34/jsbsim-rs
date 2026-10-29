// SPDX-License-Identifier: LGPL-2.1-or-later
//! The C++ side: the driver's `bench` mode (`tools/oracle/cpp/bench.cpp`).

use std::path::Path;
use std::process::Command;

use crate::OracleError;
use crate::upstream::{Paths, run_blocking};

use super::{AIRBORNE_IC, BenchCase, Workload};

/// Times one repeat of `case` in the compiled driver, run in `run_dir`;
/// returns the elapsed ns of each window it reports.
pub(super) fn time_blocking(
    paths: &Paths,
    case: &BenchCase,
    run_dir: &Path,
) -> Result<Vec<u64>, OracleError> {
    let root = &case.cpp_root(paths);
    let mut command = Command::new(paths.driver_exe());
    command.current_dir(run_dir).env("JSBSIM_DEBUG", "0").env(
        "ORACLE_AIRCRAFT_PATH",
        case.aircraft_path.as_deref().unwrap_or("aircraft"),
    );
    let workload = case.workload()?;
    match workload {
        Workload::Script(script) => command.arg("bench").arg(root).arg(root.join(script)),
        Workload::Aircraft(name) => command.arg("bench-aircraft").arg(root).arg(name),
    };
    command
        .arg(case.warm_up.to_string())
        .arg(case.frames.to_string())
        .arg("1");
    if let Workload::Aircraft(_) = workload {
        command.args(AIRBORNE_IC.map(|(property, value)| format!("{property}={value:?}")));
    }
    let output = run_blocking(&mut command).map_err(|e| OracleError::Bench {
        case: case.name.clone(),
        message: format!("C++ driver: {e}"),
    })?;
    parse_bench_output(&String::from_utf8_lossy(&output.stdout), case.frames).map_err(|message| {
        OracleError::Bench {
            case: case.name.clone(),
            message,
        }
    })
}

/// Extracts the elapsed ns of every `bench frames=<frames> ns=<ns>` line
/// from the driver's stdout, which may also carry JSBSim's load messages.
/// Fails if there is no such line or one reports other frames.
pub fn parse_bench_output(stdout: &str, frames: u32) -> Result<Vec<u64>, String> {
    let mut elapsed = Vec::new();
    for line in stdout.lines().filter(|line| line.starts_with("bench ")) {
        let mut fields = line.split_whitespace().skip(1);
        let got_frames = fields
            .next()
            .and_then(|f| f.strip_prefix("frames="))
            .and_then(|v| v.parse::<u32>().ok());
        let ns = fields
            .next()
            .and_then(|f| f.strip_prefix("ns="))
            .and_then(|v| v.parse::<u64>().ok());
        match (got_frames, ns, fields.next()) {
            (Some(f), Some(ns), None) if f == frames => elapsed.push(ns),
            _ => {
                return Err(format!(
                    "malformed driver line {line:?} for {frames} frames"
                ));
            }
        }
    }
    if elapsed.is_empty() {
        return Err(format!("driver printed no bench line:\n{stdout}"));
    }
    Ok(elapsed)
}
