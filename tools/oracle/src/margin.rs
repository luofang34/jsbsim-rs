// SPDX-License-Identifier: LGPL-2.1-or-later
//! Each tolerance row's margin, its bound over the largest difference it
//! accepts, measured over the same comparisons `port` makes, and the check
//! that the margins the rows' reasons claim hold (see [`claim`]).
//!
//! A sample counts when it differs at all: equal values (equal infinities
//! included) and NaN on both sides are no difference, as in
//! [`Tolerance::agrees`]. Its margin is `allowed / difference`, which is
//! `abs / |diff|` for an absolute row and `rel / (|diff| / |reference|)`
//! for a relative one; a row's margin is the least over its samples, and
//! infinite when it has none. A NaN on one side, or an infinity against a
//! finite value, has margin 0.

use std::fmt;

use crate::transcript::declared_unsupported;
use crate::{OracleError, Series, Tolerance, ToleranceTable};

pub mod claim;

pub use claim::Claim;

#[cfg(test)]
mod tests;

/// The least margin of one row and where it was seen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Worst {
    /// Bound over difference at that sample.
    pub margin: f64,
    /// |candidate - reference| there.
    pub difference: f64,
    /// Sample time, or line number of a transcript.
    pub at: f64,
}

/// Least margin of every row of a table.
#[derive(Debug, Clone, PartialEq)]
pub struct Margins {
    worst: Vec<Option<Worst>>,
}

impl Margins {
    /// No samples yet for any row of `table`.
    #[must_use]
    pub fn new(table: &ToleranceTable) -> Self {
        Self {
            worst: vec![None; table.rows().len()],
        }
    }

    /// The least margin of row `index`, `None` while it has seen no
    /// differing sample.
    #[must_use]
    pub fn worst(&self, index: usize) -> Option<Worst> {
        self.worst.get(index).copied().flatten()
    }

    /// Counts the sample `reference`, `candidate` at `at` under row
    /// `index`.
    pub fn record(
        &mut self,
        index: usize,
        tolerance: &Tolerance,
        reference: f64,
        candidate: f64,
        at: f64,
    ) {
        if reference == candidate || (reference.is_nan() && candidate.is_nan()) {
            return;
        }
        let difference = tolerance.difference(reference, candidate);
        if difference == 0.0 {
            return;
        }
        let margin = tolerance.allowed(reference) / difference;
        let margin = if margin.is_nan() { 0.0 } else { margin };
        let Some(slot) = self.worst.get_mut(index) else {
            return;
        };
        if slot.is_none_or(|w| margin < w.margin) {
            *slot = Some(Worst {
                margin,
                difference,
                at,
            });
        }
    }

    /// Counts every value of `candidate` against `reference` for `case`,
    /// over the rows both have.
    pub fn observe_series(
        &mut self,
        case: &str,
        reference: &Series,
        candidate: &Series,
        table: &ToleranceTable,
    ) -> Result<(), OracleError> {
        for (ref_row, cand_row) in reference.rows.iter().zip(&candidate.rows) {
            let time = ref_row.first().copied().unwrap_or(f64::NAN);
            for ((property, &r), &c) in reference.columns.iter().zip(ref_row).zip(cand_row) {
                let index = table.position_series(case, property, time).ok_or_else(|| {
                    OracleError::MissingTolerance {
                        case: case.to_owned(),
                        property: property.clone(),
                        time,
                    }
                })?;
                if let Some(tolerance) = table.rows().get(index) {
                    self.record(index, tolerance, r, c, time);
                }
            }
        }
        Ok(())
    }

    /// Counts every number of `candidate` against `reference` for `case`
    /// as `compare_transcript_declaring` pairs them; a line it would
    /// reject on its text is skipped from the first unequal text token.
    pub fn observe_transcript(
        &mut self,
        case: &str,
        reference: &str,
        candidate: &str,
        table: &ToleranceTable,
        unsupported: &[String],
    ) -> Result<(), OracleError> {
        if !table.rows().iter().any(|row| row.case == case) {
            return Ok(());
        }
        for (number, (a, b)) in reference.lines().zip(candidate.lines()).enumerate() {
            if a == b || declared_unsupported(a, b, unsupported) {
                continue;
            }
            self.observe_line(case, number.wrapping_add(1), a, b, table)?;
        }
        Ok(())
    }

    fn observe_line(
        &mut self,
        case: &str,
        line: usize,
        a: &str,
        b: &str,
        table: &ToleranceTable,
    ) -> Result<(), OracleError> {
        let (ta, tb): (Vec<&str>, Vec<&str>) = (
            a.split_whitespace().collect(),
            b.split_whitespace().collect(),
        );
        if ta.len() != tb.len() {
            return Ok(());
        }
        let op = ta.first().copied().unwrap_or_default();
        let mut tag = "";
        for (x, y) in ta.iter().zip(&tb) {
            match (x.parse::<f64>(), y.parse::<f64>()) {
                (Ok(r), Ok(c)) => {
                    if r.to_bits() == c.to_bits() || (r.is_nan() && c.is_nan()) {
                        continue;
                    }
                    let property = format!("{op}/{tag}");
                    let index = crate::transcript::position(table, case, &property, line)?;
                    if let Some(tolerance) = table.rows().get(index) {
                        self.record(index, tolerance, r, c, line as f64);
                    }
                }
                _ if x == y => tag = x,
                _ => return Ok(()),
            }
        }
        Ok(())
    }

    /// Every row of `cases` whose reason's claim its margin refutes, in
    /// table order.
    #[must_use]
    pub fn refuted(&self, table: &ToleranceTable, cases: &[String]) -> Vec<Refutation> {
        let mut out = Vec::new();
        for (index, row) in table.rows().iter().enumerate() {
            if !cases.contains(&row.case) {
                continue;
            }
            let worst = self.worst(index);
            let message = match Claim::parse(&row.reason) {
                Err(message) => Some(message),
                Ok(None) => None,
                Ok(Some(claim)) => claim.refute(worst.map_or(f64::INFINITY, |w| w.margin)),
            };
            if let Some(message) = message {
                out.push(Refutation {
                    index,
                    case: row.case.clone(),
                    property: row.property.clone(),
                    worst,
                    message,
                });
            }
        }
        out
    }
}

/// A row whose stated margin does not hold.
#[derive(Debug, Clone, PartialEq)]
pub struct Refutation {
    /// Zero-based index of the row in the table.
    pub index: usize,
    /// Case of the row.
    pub case: String,
    /// Property of the row.
    pub property: String,
    /// Its least margin, `None` when it observed no difference.
    pub worst: Option<Worst>,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for Refutation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tolerance row {} ({} {}): {}",
            self.index, self.case, self.property, self.message
        )?;
        if let Some(w) = self.worst {
            write!(f, " (max |diff| {:e} at {})", w.difference, w.at)?;
        }
        Ok(())
    }
}
