// SPDX-License-Identifier: LGPL-2.1-or-later
//! Largest Rust-vs-C++ differences of a script case, per property, for
//! choosing and justifying tolerance rows. Angles listed with a period in
//! the case's tolerance rows are compared as wrapped differences.

use std::path::Path;
use std::process::ExitCode;

use tracing::{error, info};

use crate::manifest::{CaseSpec, Manifest, REFERENCE_FILE};
use crate::{OracleError, Series};

/// Largest difference of one property up to a time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Max {
    /// Largest |Rust - C++|.
    pub abs: f64,
    /// Largest |Rust - C++| / |C++| where C++ is not zero.
    pub rel: f64,
    /// Time of the largest absolute difference, s.
    pub time: f64,
}

/// Per-property maxima of `candidate` against `reference` over the rows
/// with time at most `until`; angles with a period wrap.
#[must_use]
pub fn maxima(
    reference: &Series,
    candidate: &Series,
    until: f64,
    period: impl Fn(&str) -> Option<f64>,
) -> Vec<(String, Max)> {
    let mut out = Vec::new();
    for (i, name) in reference.columns.iter().enumerate().skip(1) {
        let p = period(name);
        let mut m = Max {
            abs: 0.0,
            rel: 0.0,
            time: 0.0,
        };
        for (r, c) in reference.rows.iter().zip(&candidate.rows) {
            let (Some(&t), Some(&a), Some(&b)) = (r.first(), r.get(i), c.get(i)) else {
                continue;
            };
            if t > until {
                break;
            }
            let mut d = b - a;
            if let Some(p) = p {
                d -= p * (d / p).round();
            }
            let d = d.abs();
            if d > m.abs || d.is_nan() {
                m.abs = d;
                m.time = t;
            }
            if a != 0.0 {
                m.rel = m.rel.max(d / a.abs());
            }
        }
        out.push((name.clone(), m));
    }
    out
}

/// Runs the Rust port on script case `case` and logs its per-property
/// maxima up to `until` seconds (the whole run when absent).
pub fn port_stats_blocking(
    repo_root: &Path,
    case: &str,
    until: Option<f64>,
) -> Result<ExitCode, OracleError> {
    let dir = repo_root.join("oracle").join("goldens").join(case);
    let manifest = Manifest::read_blocking(&dir)?;
    let CaseSpec::Script(spec) = &manifest.case else {
        return Err(OracleError::Provenance(format!(
            "{case} is not a script case"
        )));
    };
    let reference = Series::read_blocking(&dir.join(REFERENCE_FILE))?;
    let (candidate, failure) = crate::port_script::run_script(&spec.rust_root(repo_root), spec);
    let last = candidate
        .time(candidate.rows.len().saturating_sub(1))
        .unwrap_or(f64::NAN);
    if let Some(e) = &failure {
        error!("{case}: the Rust run stopped after t={last}: {e}");
    }
    let out = repo_root.join("target").join("oracle").join("port");
    std::fs::create_dir_all(&out).map_err(|e| OracleError::io("creating", &out, e))?;
    candidate.write_blocking(&out.join(format!("{case}.csv")))?;
    let table =
        crate::ToleranceTable::read_blocking(&repo_root.join("oracle").join("tolerances.toml"))?;
    let until = until.unwrap_or(f64::INFINITY);
    info!(
        "{case}: {} reference rows, {} Rust rows, compared to t={}",
        reference.rows.len(),
        candidate.rows.len(),
        until.min(last)
    );
    for (name, m) in maxima(&reference, &candidate, until, |p| {
        table.lookup(case, p, 0.0).and_then(|t| t.period)
    }) {
        info!(
            "{case} {name:<36} max |diff| {:.3e} at t={:.2}, max rel {:.3e}",
            m.abs, m.time, m.rel
        );
    }
    Ok(ExitCode::SUCCESS)
}
