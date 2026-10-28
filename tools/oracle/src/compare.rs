// SPDX-License-Identifier: LGPL-2.1-or-later
//! Comparison of a candidate series against a reference series.
//!
//! Rows are matched by position and scanned in time order; within a row the
//! columns are checked in header order, starting with `Time`, so the first
//! divergence reported is the earliest one.

use std::fmt;

use crate::{OracleError, Series, Tolerance, ToleranceTable};

#[cfg(test)]
mod tests;

/// The first point at which the candidate leaves its tolerance.
#[derive(Debug, Clone, PartialEq)]
pub enum Divergence {
    /// A value differs by more than its tolerance.
    Value {
        /// Reference sample time.
        time: f64,
        /// Column that diverged.
        property: String,
        /// Reference value.
        reference: f64,
        /// Candidate value.
        candidate: f64,
        /// Tolerance row that was applied.
        tolerance: Tolerance,
    },
    /// One series has more rows than the other.
    RowCount {
        /// Time of the first row present in only one series.
        time: f64,
        /// Rows in the reference.
        reference_rows: usize,
        /// Rows in the candidate.
        candidate_rows: usize,
    },
}

impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value {
                time,
                property,
                reference,
                candidate,
                tolerance,
            } => write!(
                f,
                "t={time} {property}: reference={reference:?} candidate={candidate:?} \
                 |diff|={:?} allowed={:?} (abs={:?} rel={:?}: {})",
                (candidate - reference).abs(),
                tolerance.allowed(*reference),
                tolerance.abs,
                tolerance.rel,
                tolerance.reason
            ),
            Self::RowCount {
                time,
                reference_rows,
                candidate_rows,
            } => write!(
                f,
                "t={time}: reference has {reference_rows} rows, candidate has {candidate_rows}"
            ),
        }
    }
}

/// Result of a comparison that ran to completion.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// Rows checked before stopping.
    pub rows_compared: usize,
    /// Columns per row, including `Time`.
    pub columns: usize,
    /// First divergence, or `None` if the candidate agrees everywhere.
    pub divergence: Option<Divergence>,
}

/// Compares `candidate` against `reference` with the rows of `table` that
/// belong to `case`.
///
/// Fails if the columns differ or a column has no tolerance row at a sample
/// time: an unlisted property is a configuration error, not a pass.
pub fn compare(
    case: &str,
    reference: &Series,
    candidate: &Series,
    table: &ToleranceTable,
) -> Result<Outcome, OracleError> {
    if reference.columns != candidate.columns {
        return Err(OracleError::ColumnMismatch {
            reference: reference.columns.clone(),
            candidate: candidate.columns.clone(),
        });
    }
    let columns = reference.columns.len();
    for (index, (ref_row, cand_row)) in reference.rows.iter().zip(&candidate.rows).enumerate() {
        let time = ref_row.first().copied().unwrap_or(f64::NAN);
        for ((property, &r), &c) in reference.columns.iter().zip(ref_row).zip(cand_row) {
            let tolerance = table.lookup(case, property, time).ok_or_else(|| {
                OracleError::MissingTolerance {
                    case: case.to_owned(),
                    property: property.clone(),
                    time,
                }
            })?;
            if !agrees(r, c, tolerance) {
                return Ok(Outcome {
                    rows_compared: index.wrapping_add(1),
                    columns,
                    divergence: Some(Divergence::Value {
                        time,
                        property: property.clone(),
                        reference: r,
                        candidate: c,
                        tolerance: tolerance.clone(),
                    }),
                });
            }
        }
    }
    let shared = reference.rows.len().min(candidate.rows.len());
    let divergence = (reference.rows.len() != candidate.rows.len()).then(|| Divergence::RowCount {
        time: reference
            .time(shared)
            .or_else(|| candidate.time(shared))
            .unwrap_or(f64::NAN),
        reference_rows: reference.rows.len(),
        candidate_rows: candidate.rows.len(),
    });
    Ok(Outcome {
        rows_compared: shared,
        columns,
        divergence,
    })
}

/// Exact equality (which covers infinities), both NaN, or a difference
/// within the bound. A NaN on one side only never agrees.
fn agrees(reference: f64, candidate: f64, tolerance: &Tolerance) -> bool {
    reference == candidate
        || (reference.is_nan() && candidate.is_nan())
        || (candidate - reference).abs() <= tolerance.allowed(reference)
}
