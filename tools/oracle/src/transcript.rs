// SPDX-License-Identifier: LGPL-2.1-or-later
//! Comparison of probe transcripts.
//!
//! A case without rows in the tolerance table must match byte for byte.
//! Otherwise lines are compared token by token: text exactly, numbers with
//! the row for `<op>/<tag>` (the line's first word and the nearest text
//! token before the number), falling back to `<op>/*` and then to the
//! case's `*` row. The line number plays the role of time in the table.

use std::fmt;

use crate::{OracleError, Tolerance, ToleranceTable};

#[cfg(test)]
mod tests;

/// The first point at which two transcripts disagree.
#[derive(Debug, Clone, PartialEq)]
pub struct LineDivergence {
    /// One-based line number.
    pub line: usize,
    /// What differs.
    pub detail: String,
}

impl fmt::Display for LineDivergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.detail)
    }
}

/// Result of a transcript comparison.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TranscriptOutcome {
    /// Lines compared.
    pub lines: usize,
    /// Numbers that differ at all, within tolerance or not.
    pub numbers_differing: usize,
    /// Largest |candidate - reference| over all numbers.
    pub max_abs: f64,
    /// Largest |candidate - reference| / |reference| over all numbers.
    pub max_rel: f64,
    /// First disagreement beyond tolerance.
    pub divergence: Option<LineDivergence>,
}

/// Compares `candidate` with `reference` for `case`.
pub fn compare_transcript(
    case: &str,
    reference: &str,
    candidate: &str,
    table: &ToleranceTable,
) -> Result<TranscriptOutcome, OracleError> {
    let numeric = table.rows().iter().any(|row| row.case == case);
    let mut outcome = TranscriptOutcome::default();
    let mut left = reference.lines();
    let mut right = candidate.lines();
    loop {
        let (a, b) = (left.next(), right.next());
        if a.is_none() && b.is_none() {
            return Ok(outcome);
        }
        outcome.lines = outcome.lines.wrapping_add(1);
        let (a, b) = (a.unwrap_or_default(), b.unwrap_or_default());
        if a == b {
            continue;
        }
        let detail = if numeric {
            compare_line(case, outcome.lines, a, b, table, &mut outcome)?
        } else {
            Some(format!("reference {a:?}, candidate {b:?}"))
        };
        if let Some(detail) = detail {
            outcome.divergence = Some(LineDivergence {
                line: outcome.lines,
                detail,
            });
            return Ok(outcome);
        }
    }
}

fn compare_line(
    case: &str,
    line: usize,
    a: &str,
    b: &str,
    table: &ToleranceTable,
    outcome: &mut TranscriptOutcome,
) -> Result<Option<String>, OracleError> {
    let (ta, tb): (Vec<&str>, Vec<&str>) = (
        a.split_whitespace().collect(),
        b.split_whitespace().collect(),
    );
    if ta.len() != tb.len() {
        return Ok(Some(format!("reference {a:?}, candidate {b:?}")));
    }
    let op = ta.first().copied().unwrap_or_default();
    let mut tag = "";
    for (x, y) in ta.iter().zip(&tb) {
        let (nx, ny) = (x.parse::<f64>(), y.parse::<f64>());
        match (nx, ny) {
            (Ok(r), Ok(c)) => {
                if r.to_bits() == c.to_bits() || (r.is_nan() && c.is_nan()) {
                    continue;
                }
                let property = format!("{op}/{tag}");
                let tolerance = lookup(table, case, &property, line)?;
                outcome.numbers_differing = outcome.numbers_differing.wrapping_add(1);
                let diff = tolerance.difference(r, c);
                outcome.max_abs = outcome.max_abs.max(diff);
                if r != 0.0 {
                    outcome.max_rel = outcome.max_rel.max(diff / r.abs());
                }
                if !tolerance.agrees(r, c) {
                    return Ok(Some(format!(
                        "{property}: reference={r:?} candidate={c:?} |diff|={diff:?} allowed={:?} ({})",
                        tolerance.allowed(r),
                        tolerance.reason
                    )));
                }
            }
            _ if x == y => tag = x,
            _ => return Ok(Some(format!("token {x:?} vs {y:?}"))),
        }
    }
    Ok(None)
}

fn lookup<'t>(
    table: &'t ToleranceTable,
    case: &str,
    property: &str,
    line: usize,
) -> Result<&'t Tolerance, OracleError> {
    let time = line as f64;
    let op_wide = format!("{}/*", property.split('/').next().unwrap_or_default());
    table
        .lookup(case, property, time)
        .or_else(|| table.lookup(case, &op_wide, time))
        .or_else(|| table.lookup(case, "*", time))
        .ok_or_else(|| OracleError::MissingTolerance {
            case: case.to_owned(),
            property: property.to_owned(),
            time,
        })
}
