// SPDX-License-Identifier: LGPL-2.1-or-later
//! Time series stored as CSV: a header row naming the columns (the first is
//! `Time`) followed by one row of numbers per sample.

use std::fmt::Write as _;
use std::path::Path;

use crate::OracleError;

#[cfg(test)]
mod tests;

/// Name of the first column of every series.
pub const TIME_COLUMN: &str = "Time";

/// Sampled property values, one row per sample time.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    /// Column names; the first is [`TIME_COLUMN`], the rest are property paths.
    pub columns: Vec<String>,
    /// Samples; every row has one value per column.
    pub rows: Vec<Vec<f64>>,
}

impl Series {
    /// Parses CSV text. `source_name` labels errors.
    pub fn parse(text: &str, source_name: &str) -> Result<Self, OracleError> {
        let csv_err = |line: usize, message: String| OracleError::Csv {
            source_name: source_name.to_owned(),
            line,
            message,
        };
        let mut lines = text.lines().enumerate();
        let header = lines
            .next()
            .map(|(_, line)| line)
            .ok_or_else(|| csv_err(1, "empty file".to_owned()))?;
        let columns: Vec<String> = header.split(',').map(|c| c.trim().to_owned()).collect();
        if columns.first().map(String::as_str) != Some(TIME_COLUMN) {
            return Err(csv_err(1, format!("first column must be {TIME_COLUMN}")));
        }
        let mut rows = Vec::new();
        for (index, line) in lines {
            if line.trim().is_empty() {
                continue;
            }
            let row = line
                .split(',')
                .map(|cell| {
                    cell.trim()
                        .parse::<f64>()
                        .map_err(|e| csv_err(index + 1, format!("{cell:?}: {e}")))
                })
                .collect::<Result<Vec<f64>, _>>()?;
            if row.len() != columns.len() {
                return Err(csv_err(
                    index + 1,
                    format!("{} cells, header has {}", row.len(), columns.len()),
                ));
            }
            rows.push(row);
        }
        Ok(Self { columns, rows })
    }

    /// Reads and parses a CSV file.
    pub fn read_blocking(path: &Path) -> Result<Self, OracleError> {
        let text =
            std::fs::read_to_string(path).map_err(|e| OracleError::io("reading", path, e))?;
        Self::parse(&text, &path.display().to_string())
    }

    /// Formats the series as CSV with values that round-trip exactly.
    #[must_use]
    pub fn to_csv(&self) -> String {
        let mut out = self.columns.join(",");
        out.push('\n');
        for row in &self.rows {
            for (i, value) in row.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                // Rust's shortest round-trip formatting cannot fail on a String.
                write!(out, "{value:?}").ok();
            }
            out.push('\n');
        }
        out
    }

    /// Writes the series to a CSV file.
    pub fn write_blocking(&self, path: &Path) -> Result<(), OracleError> {
        std::fs::write(path, self.to_csv()).map_err(|e| OracleError::io("writing", path, e))
    }

    /// Sample time of row `index`, if the row exists.
    #[must_use]
    pub fn time(&self, index: usize) -> Option<f64> {
        self.rows.get(index).and_then(|row| row.first().copied())
    }
}
