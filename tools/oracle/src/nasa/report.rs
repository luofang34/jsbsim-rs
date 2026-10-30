// SPDX-License-Identifier: LGPL-2.1-or-later
//! Verdicts of one NASA case and their counts. Every expected miss must
//! both miss and name a compared variable, so the manifest can neither
//! keep an entry that stopped applying nor carry a misspelt one.

use std::fmt;
use std::ops::AddAssign;

use tracing::{error, info};

use super::data::{Column, Simulator};
use super::envelope::{ColumnStats, compare_column, samples};
use super::spread::{spread_ratio, two_figures};
use super::{Case, ExpectedMiss};
use crate::Series;

/// How many verdicts of each kind a case (or a run) produced. A compared
/// variable counts once, as `pass`, `expected_miss`, `unexpected_pass` or
/// `fail`; one the Rust port misses on other samples than C++ is a `fail`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    /// Inside the simulators at every sample, and not listed.
    pub pass: usize,
    /// Outside, and listed with its cause.
    pub expected_miss: usize,
    /// Listed as a miss but inside at every sample.
    pub unexpected_pass: usize,
    /// Outside and not listed, or missed by Rust on other samples.
    pub fail: usize,
    /// Expected misses naming a variable the case does not compare.
    pub stale: usize,
    /// Properties whose Rust-vs-C++ difference exceeds the case's bound,
    /// plus one when the runs' row counts differ.
    pub parity_fail: usize,
    /// Cases whose Rust run stopped before the end.
    pub rust_stopped: usize,
}

impl Counts {
    /// Whether nothing failed.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.unexpected_pass == 0
            && self.fail == 0
            && self.stale == 0
            && self.parity_fail == 0
            && self.rust_stopped == 0
    }
}

impl AddAssign for Counts {
    fn add_assign(&mut self, other: Self) {
        self.pass = self.pass.wrapping_add(other.pass);
        self.expected_miss = self.expected_miss.wrapping_add(other.expected_miss);
        self.unexpected_pass = self.unexpected_pass.wrapping_add(other.unexpected_pass);
        self.fail = self.fail.wrapping_add(other.fail);
        self.stale = self.stale.wrapping_add(other.stale);
        self.parity_fail = self.parity_fail.wrapping_add(other.parity_fail);
        self.rust_stopped = self.rust_stopped.wrapping_add(other.rust_stopped);
    }
}

impl fmt::Display for Counts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} pass, {} expected_miss, {} unexpected_pass, {} fail, \
             {} stale expected_miss, {} parity fail, {} Rust run stopped",
            self.pass,
            self.expected_miss,
            self.unexpected_pass,
            self.fail,
            self.stale,
            self.parity_fail,
            self.rust_stopped
        )
    }
}

/// Counts `c`'s verdict into `counts` and returns its label.
fn verdict(c: &ColumnStats, expected: Option<&ExpectedMiss>, counts: &mut Counts) -> String {
    match (c.passes(), expected) {
        (true, None) => {
            counts.pass = counts.pass.wrapping_add(1);
            "PASS".to_owned()
        }
        (false, Some(m)) => {
            counts.expected_miss = counts.expected_miss.wrapping_add(1);
            format!("EXPECTED MISS ({})", m.cause)
        }
        (true, Some(_)) => {
            counts.unexpected_pass = counts.unexpected_pass.wrapping_add(1);
            "FAIL: listed as an expected miss but inside; update oracle/nasa/manifest.toml"
                .to_owned()
        }
        (false, None) => {
            counts.fail = counts.fail.wrapping_add(1);
            "FAIL".to_owned()
        }
    }
}

/// Logs each variable of `columns`' verdict: C++ inside the simulators at
/// every sample, or missing where the manifest names the cause; Rust
/// outside on exactly the same samples. Expected misses of variables not
/// compared are logged and counted as stale.
pub(super) fn report_case(
    case: &Case,
    columns: &[Column],
    simulators: &[Simulator],
    cpp: &Series,
    rust: &Series,
) -> Counts {
    let mut counts = Counts::default();
    let mut compared: Vec<&str> = Vec::new();
    let (mut within_spread, mut spread_compared) = (0usize, 0usize);
    for column in columns {
        let (Some(c), Some(r)) = (
            compare_column(column, simulators, cpp),
            compare_column(column, simulators, rust),
        ) else {
            continue;
        };
        compared.push(column.nasa);
        let ratio = samples(column, cpp)
            .and_then(|s| spread_ratio(column, simulators, &s))
            .map(|r| r.ratio());
        if let Some(r) = ratio {
            spread_compared = spread_compared.wrapping_add(1);
            within_spread = within_spread.wrapping_add(usize::from(r <= 1.0));
        }
        let expected = case.expected_miss.iter().find(|m| m.variable == c.nasa);
        let same = c.outside == r.outside;
        let label = if same {
            verdict(&c, expected, &mut counts)
        } else {
            counts.fail = counts.fail.wrapping_add(1);
            "FAIL".to_owned()
        };
        info!(
            "{} {:<33} sims {}, spread <= {:.3e}, C++ outside <= {:.3e} at t={:.4}, R {}, inside {}/{}; Rust inside {}/{}, {}: {label}",
            case.name,
            c.nasa,
            c.simulators,
            c.max_spread,
            c.max_excursion,
            c.worst_time,
            ratio.map_or_else(|| "n/a".to_owned(), two_figures),
            c.inside,
            c.samples,
            r.inside,
            r.samples,
            if same {
                "same samples"
            } else {
                "DIFFERENT samples"
            },
        );
    }
    info!(
        "{}: C++ within the simulators' own spread (R <= 1) on {within_spread}/{spread_compared} variables",
        case.name
    );
    for miss in &case.expected_miss {
        if !compared.contains(&miss.variable.as_str()) {
            counts.stale = counts.stale.wrapping_add(1);
            error!(
                "{} {}: FAIL: listed as an expected miss but not compared in this case; \
                 update oracle/nasa/manifest.toml",
                case.name, miss.variable
            );
        }
    }
    counts
}

/// Logs the largest Rust-vs-C++ difference of each property; returns how
/// many exceed `rel` of their magnitude, plus one if the row counts differ.
pub(super) fn report_parity(case: &str, rel: f64, cpp: &Series, rust: &Series) -> usize {
    let mut failures = usize::from(cpp.rows.len() != rust.rows.len());
    if failures != 0 {
        error!(
            "{case} parity: C++ has {} rows, Rust {}",
            cpp.rows.len(),
            rust.rows.len()
        );
    }
    for (i, property) in cpp.columns.iter().enumerate().skip(1) {
        let (mut max, mut magnitude) = (0.0_f64, 1.0_f64);
        for (a, b) in cpp.rows.iter().zip(&rust.rows) {
            if let (Some(a), Some(b)) = (a.get(i), b.get(i)) {
                max = max.max((a - b).abs());
                magnitude = magnitude.max(a.abs());
            }
        }
        let ok = max <= rel * magnitude;
        if !ok {
            failures = failures.wrapping_add(1);
        }
        info!(
            "{case} parity {property:<33} max |Rust - C++| {max:.3e}, {:.3e} of its magnitude{}",
            max / magnitude,
            if ok { "" } else { " FAIL" }
        );
    }
    failures
}
