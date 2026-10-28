// SPDX-License-Identifier: LGPL-2.1-or-later
//! Per-property tolerance table.
//!
//! Each row bounds |candidate - reference| by `abs + rel * |reference|` for
//! one property of one case, optionally only inside a half-open time window
//! `[t_start, t_end)`. Every row must state its reason, so any loosening is
//! justified in the same place it is made.

use std::path::Path;

use serde::Deserialize;

use crate::OracleError;

#[cfg(test)]
mod tests;

/// One row of the tolerance table.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tolerance {
    /// Golden case the row applies to.
    pub case: String,
    /// Column (property path, or `Time`) the row applies to.
    pub property: String,
    /// Absolute part of the bound, in the property's unit.
    pub abs: f64,
    /// Relative part of the bound, as a fraction of |reference|.
    pub rel: f64,
    /// For angles on a branch cut: differences are wrapped into
    /// `[-period/2, period/2]` before the bound applies (2π or 360).
    #[serde(default)]
    pub period: Option<f64>,
    /// Inclusive start of the window; unbounded when absent.
    #[serde(default)]
    pub t_start: Option<f64>,
    /// Exclusive end of the window; unbounded when absent.
    #[serde(default)]
    pub t_end: Option<f64>,
    /// Why the bound has this size.
    pub reason: String,
}

impl Tolerance {
    /// Whether the row's time window contains `time`.
    #[must_use]
    pub fn covers(&self, time: f64) -> bool {
        self.t_start.is_none_or(|start| time >= start) && self.t_end.is_none_or(|end| time < end)
    }

    /// |candidate - reference|, wrapped by the period when there is one.
    #[must_use]
    pub fn difference(&self, reference: f64, candidate: f64) -> f64 {
        let d = candidate - reference;
        match self.period {
            Some(p) => (d - p * (d / p).round()).abs(),
            None => d.abs(),
        }
    }

    /// Whether `candidate` agrees with `reference`: exactly (which covers
    /// infinities), both NaN, or within the bound. A NaN on one side only
    /// never agrees.
    #[must_use]
    pub fn agrees(&self, reference: f64, candidate: f64) -> bool {
        reference == candidate
            || (reference.is_nan() && candidate.is_nan())
            || self.difference(reference, candidate) <= self.allowed(reference)
    }

    /// Largest accepted |candidate - reference| for `reference`.
    #[must_use]
    pub fn allowed(&self, reference: f64) -> f64 {
        self.abs + self.rel * reference.abs()
    }

    fn validate(&self, index: usize) -> Result<(), OracleError> {
        let fail = |message: &str| {
            Err(OracleError::Tolerance {
                index,
                case: self.case.clone(),
                property: self.property.clone(),
                message: message.to_owned(),
            })
        };
        if self.reason.trim().is_empty() {
            return fail("reason must not be empty");
        }
        if !(self.abs.is_finite() && self.abs >= 0.0 && self.rel.is_finite() && self.rel >= 0.0) {
            return fail("abs and rel must be finite and non-negative");
        }
        if self.t_start.is_some_and(f64::is_nan) || self.t_end.is_some_and(f64::is_nan) {
            return fail("t_start and t_end must not be NaN");
        }
        if self.period.is_some_and(|p| !(p.is_finite() && p > 0.0)) {
            return fail("period must be finite and positive");
        }
        if let (Some(start), Some(end)) = (self.t_start, self.t_end)
            && start.partial_cmp(&end) != Some(std::cmp::Ordering::Less)
        {
            return fail("t_start must be less than t_end");
        }
        Ok(())
    }

    fn window(&self) -> (f64, f64) {
        (
            self.t_start.unwrap_or(f64::NEG_INFINITY),
            self.t_end.unwrap_or(f64::INFINITY),
        )
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TableFile {
    #[serde(default)]
    tolerance: Vec<Tolerance>,
}

/// Validated tolerance table.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToleranceTable {
    rows: Vec<Tolerance>,
}

impl ToleranceTable {
    /// Builds a table from rows, rejecting invalid rows and overlapping
    /// windows for the same case and property.
    pub fn new(rows: Vec<Tolerance>) -> Result<Self, OracleError> {
        for (index, row) in rows.iter().enumerate() {
            row.validate(index)?;
            let overlapping = rows.iter().take(index).any(|earlier| {
                let (a0, a1) = earlier.window();
                let (b0, b1) = row.window();
                earlier.case == row.case && earlier.property == row.property && a0 < b1 && b0 < a1
            });
            if overlapping {
                return Err(OracleError::Tolerance {
                    index,
                    case: row.case.clone(),
                    property: row.property.clone(),
                    message: "time window overlaps an earlier row".to_owned(),
                });
            }
        }
        Ok(Self { rows })
    }

    /// Parses TOML made of `[[tolerance]]` tables. `path` labels errors.
    pub fn parse(text: &str, path: &Path) -> Result<Self, OracleError> {
        let file: TableFile = toml::from_str(text).map_err(|e| OracleError::Toml {
            path: path.to_owned(),
            source: Box::new(e),
        })?;
        Self::new(file.tolerance)
    }

    /// Reads and parses a tolerance file.
    pub fn read_blocking(path: &Path) -> Result<Self, OracleError> {
        let text =
            std::fs::read_to_string(path).map_err(|e| OracleError::io("reading", path, e))?;
        Self::parse(&text, path)
    }

    /// The row of `case` named exactly `property` covering `time`, if any.
    #[must_use]
    pub fn lookup(&self, case: &str, property: &str, time: f64) -> Option<&Tolerance> {
        self.rows
            .iter()
            .find(|row| row.case == case && row.property == property && row.covers(time))
    }

    /// The row bounding series column `property` of `case` at `time`: the
    /// column's own row, else the case's `*` row, which bounds every
    /// column the case does not list.
    #[must_use]
    pub fn lookup_series(&self, case: &str, property: &str, time: f64) -> Option<&Tolerance> {
        self.lookup(case, property, time)
            .or_else(|| self.lookup(case, "*", time))
    }

    /// All rows, in file order.
    #[must_use]
    pub fn rows(&self) -> &[Tolerance] {
        &self.rows
    }
}
