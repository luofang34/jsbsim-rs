// SPDX-License-Identifier: LGPL-2.1-or-later
//! The `check-sockets` check, run by scripts/oracle-ci.sh: every socket
//! the C++ driver binds is on loopback.
//!
//! The 737 and tripod_input declare `<input>` sockets, which JSBSim binds
//! to every interface; the driver's `bind` (tools/oracle/cpp/loopback_bind.cpp)
//! moves them to loopback and, given [`REPORT_ENV`], records each call.
//! The check runs those cases with the report on and fails when a bind
//! was sent or landed off loopback, or an expected port was never bound,
//! which is what happens when the driver's `bind` is missing.

use std::net::IpAddr;
use std::process::ExitCode;

use tracing::{error, info};

use crate::OracleError;
use crate::manifest::{self, CaseSpec, Manifest};
use crate::upstream::{self, Paths};

#[cfg(test)]
mod tests;

/// Environment variable naming the file the driver appends its binds to.
pub const REPORT_ENV: &str = "ORACLE_SOCKET_REPORT";

/// Golden cases with `<input>` sockets, the ports their aircraft declare,
/// and the time to stop at, the script's end when `None`. tripod_input's
/// `<input>` without a port still gets a TCP socket, bound to port 0, so
/// to a port of the system's choosing.
pub const CASES: &[(&str, &[u16], Option<f64>)] = &[
    ("tripod_input", &[0, 5139], None),
    ("737_cruise", &[5137, 5139], Some(2.0)),
];

/// One line of the driver's report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bind {
    /// Address and port passed to the C library's `bind`; `None` for a
    /// family other than AF_INET and AF_INET6.
    pub sent: Option<(IpAddr, u16)>,
    /// Whether the bind succeeded.
    pub ok: bool,
    /// Address and port `getsockname` gave after a successful bind.
    pub bound: Option<(IpAddr, u16)>,
}

fn parse_address(text: &str) -> Option<(IpAddr, u16)> {
    let (ip, port) = text.rsplit_once(':')?;
    Some((ip.parse().ok()?, port.parse().ok()?))
}

fn parse_line(line: &str) -> Option<Bind> {
    let mut fields = line.split_whitespace();
    if fields.next()? != "bind" {
        return None;
    }
    fields.next()?;
    let sent = fields.next()?.strip_prefix("sent=")?;
    let result = fields.next()?.strip_prefix("result=")?;
    let bound = fields.next()?.strip_prefix("bound=")?;
    Some(Bind {
        sent: parse_address(sent),
        ok: result == "0",
        bound: parse_address(bound),
    })
}

/// Parses the driver's report.
pub fn parse_report(text: &str) -> Result<Vec<Bind>, OracleError> {
    text.lines()
        .map(|line| {
            parse_line(line).ok_or_else(|| {
                OracleError::Provenance(format!("malformed socket report line {line:?}"))
            })
        })
        .collect()
}

/// What is wrong with `binds`, a run expected to bind each of `ports`.
#[must_use]
pub fn problems(binds: &[Bind], ports: &[u16]) -> Vec<String> {
    let mut found = Vec::new();
    for bind in binds {
        match bind.sent {
            Some((ip, port)) if ip.is_loopback() => {
                if bind.ok && !bind.bound.is_some_and(|(ip, _)| ip.is_loopback()) {
                    found.push(format!("{bind:?}: bound off loopback"));
                }
                if !ports.contains(&port) {
                    found.push(format!("{bind:?}: port {port} is not declared"));
                }
            }
            _ => found.push(format!("{bind:?}: sent off loopback")),
        }
    }
    for port in ports {
        if !binds
            .iter()
            .any(|b| b.sent.is_some_and(|(_, p)| p == *port))
        {
            found.push(format!("port {port} was never bound"));
        }
    }
    found
}

/// Runs [`CASES`] through the driver and checks their binds.
pub fn check_blocking(paths: &Paths) -> Result<ExitCode, OracleError> {
    let mut code = ExitCode::SUCCESS;
    for &(case, ports, end_time) in CASES {
        let binds = run_case_blocking(paths, case, end_time)?;
        for bind in &binds {
            info!("{case}: {bind:?}");
        }
        let found = problems(&binds, ports);
        for problem in &found {
            error!("{case}: {problem}");
        }
        if found.is_empty() {
            info!("{case}: {} binds, all on loopback", binds.len());
        } else {
            code = ExitCode::FAILURE;
        }
    }
    Ok(code)
}

fn run_case_blocking(
    paths: &Paths,
    case: &str,
    end_time: Option<f64>,
) -> Result<Vec<Bind>, OracleError> {
    let manifest = Manifest::read_blocking(&manifest::case_dir(&paths.goldens_dir(), case))?;
    let CaseSpec::Script(mut spec) = manifest.case else {
        return Err(OracleError::Provenance(format!(
            "{case} is not a script case"
        )));
    };
    if end_time.is_some() {
        spec.end_time = end_time;
    }
    let root = spec.cpp_root(&paths.repo_root, &paths.jsbsim_root);
    let run_dir = paths.work_dir().join("run").join("sockets").join(case);
    let report = run_dir.join("sockets.txt");
    let mut driver = upstream::script_command_blocking(paths, &root, &spec, &run_dir)?;
    match std::fs::remove_file(&report) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(OracleError::io("removing", &report, e));
        }
        _ => {}
    }
    upstream::run_blocking(driver.env(REPORT_ENV, &report))?;
    let text = match std::fs::read_to_string(&report) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        read => read.map_err(|e| OracleError::io("reading", &report, e))?,
    };
    parse_report(&text)
}
