// SPDX-License-Identifier: LGPL-2.1-or-later
//! Ceilings on a case's Rust/C++ ratio (`oracle bench --max-ratio
//! case=ratio`), which scripts/oracle-bench.sh sets for c172x under each
//! release profile.

use super::CaseResult;
use super::summary::ratio;
use crate::OracleError;

/// The most the median Rust ns/frame of `case` may be over C++'s.
#[derive(Debug, Clone, PartialEq)]
pub struct Guard {
    /// Bench case name.
    pub case: String,
    /// Largest accepted Rust/C++ ratio of the medians.
    pub max_ratio: f64,
}

impl Guard {
    /// Parses `case=ratio`.
    pub fn parse(text: &str) -> Result<Self, OracleError> {
        let invalid = || OracleError::Bench {
            case: text.to_owned(),
            message: "a ratio ceiling is case=ratio with a positive ratio".to_owned(),
        };
        let (case, max) = text.split_once('=').ok_or_else(invalid)?;
        let max_ratio: f64 = max.trim().parse().map_err(|_| invalid())?;
        if case.trim().is_empty() || max_ratio.is_nan() || max_ratio <= 0.0 {
            return Err(invalid());
        }
        Ok(Self {
            case: case.trim().to_owned(),
            max_ratio,
        })
    }
}

/// How one guard fared.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// The case's ratio is at most its ceiling.
    Within {
        /// Rust/C++ of the medians.
        ratio: f64,
    },
    /// The case's ratio is over its ceiling.
    Over {
        /// Rust/C++ of the medians.
        ratio: f64,
    },
    /// The case did not run (not selected, or it failed).
    NotRun,
}

/// The verdict of `guard` on `results`.
#[must_use]
pub fn check(guard: &Guard, results: &[CaseResult]) -> Verdict {
    let Some(result) = results.iter().find(|r| r.name == guard.case) else {
        return Verdict::NotRun;
    };
    let ratio = ratio(result.rust.median, result.cpp.median);
    if ratio <= guard.max_ratio {
        Verdict::Within { ratio }
    } else {
        Verdict::Over { ratio }
    }
}
