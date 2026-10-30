// SPDX-License-Identifier: LGPL-2.1-or-later
//! Verdicts of one NASA case and their counts. Every expected miss must
//! both miss and name a compared variable, so the manifest can neither
//! keep an entry that stopped applying nor carry a misspelt one.

use std::fmt;
use std::ops::AddAssign;

use tracing::{error, info};

use super::data::{Column, Simulator};
use super::envelope::{ColumnStats, compare_column, samples};
use super::flip::accepted_flips;
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
    /// Expected misses naming a variable the case does not compare, and
    /// threshold-flip caps above the flips accepted.
    pub stale: usize,
    /// Properties whose Rust-vs-C++ difference exceeds the case's bound,
    /// plus one when the runs' row counts differ.
    pub parity_fail: usize,
    /// Cases whose Rust run stopped before the end.
    pub rust_stopped: usize,
    /// Samples accepted although Rust and C++ fall on opposite sides of
    /// the pass threshold (see `super::flip`).
    pub threshold_flips: usize,
    /// Accepted threshold flips beyond the cases' `max_threshold_flips`.
    pub flips_over_cap: usize,
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
            && self.flips_over_cap == 0
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
        self.threshold_flips = self.threshold_flips.wrapping_add(other.threshold_flips);
        self.flips_over_cap = self.flips_over_cap.wrapping_add(other.flips_over_cap);
    }
}

impl fmt::Display for Counts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} pass, {} expected_miss, {} unexpected_pass, {} fail, \
             {} stale expected_miss, {} parity fail, {} Rust run stopped, \
             {} threshold flips accepted, {} over the cap",
            self.pass,
            self.expected_miss,
            self.unexpected_pass,
            self.fail,
            self.stale,
            self.parity_fail,
            self.rust_stopped,
            self.threshold_flips,
            self.flips_over_cap
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
        let flips = agreement(case, column, simulators, (cpp, rust), (&c, &r));
        let label = if let Some(n) = flips {
            counts.threshold_flips = counts.threshold_flips.wrapping_add(n);
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
            ratio.map_or_else(
                || "n/a".to_owned(),
                |r| format!(
                    "{} ({} the simulators' own spread)",
                    two_figures(r),
                    if r <= 1.0 { "within" } else { "beyond" }
                )
            ),
            c.inside,
            c.samples,
            r.inside,
            r.samples,
            match flips {
                Some(0) => "same samples".to_owned(),
                Some(n) => format!("same samples but {n} within the parity bound of the threshold"),
                None => "DIFFERENT samples".to_owned(),
            },
        );
    }
    info!(
        "{}: C++ within the simulators' own spread (R <= 1) on {within_spread}/{spread_compared} variables",
        case.name
    );
    report_cap_and_stale(case, &compared, &mut counts);
    counts
}

/// How many samples Rust's and C++'s verdicts differ on, each logged, when
/// all are accepted threshold flips; `None` when any is not.
fn agreement(
    case: &Case,
    column: &Column,
    simulators: &[Simulator],
    runs: (&Series, &Series),
    (c, r): (&ColumnStats, &ColumnStats),
) -> Option<usize> {
    if c.outside == r.outside {
        return Some(0);
    }
    let flips = accepted_flips(
        column,
        simulators,
        runs,
        (&c.outside, &r.outside),
        case.parity_rel(),
    )?;
    for f in &flips {
        info!(
            "{} {} threshold flip at t={:.4}: C++ {:.3e} from the threshold, Rust {:.3e} from C++, bound {:.3e}",
            case.name,
            c.nasa,
            f.key as f64 / 1000.0,
            f.cpp_to_threshold,
            f.rust_to_cpp,
            f.bound
        );
    }
    Some(flips.len())
}

/// Fails accepted threshold flips other than the case's cap, so that the
/// cap stays current, and expected misses of variables not `compared`.
fn report_cap_and_stale(case: &Case, compared: &[&str], counts: &mut Counts) {
    let cap = case.max_threshold_flips();
    if counts.threshold_flips > cap {
        counts.flips_over_cap = counts.threshold_flips - cap;
        error!(
            "{}: {} threshold flips accepted, {cap} allowed by max_threshold_flips: FAIL; \
             review them and update oracle/nasa/manifest.toml",
            case.name, counts.threshold_flips
        );
    } else if counts.threshold_flips < cap {
        counts.stale = counts.stale.wrapping_add(1);
        error!(
            "{}: {} threshold flips accepted, but max_threshold_flips allows {cap}: FAIL; \
             update oracle/nasa/manifest.toml",
            case.name, counts.threshold_flips
        );
    }
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
