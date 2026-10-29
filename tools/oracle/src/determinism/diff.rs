// SPDX-License-Identifier: LGPL-2.1-or-later
//! Comparing two sets of `determinism` records, value by value as bits.

use std::path::Path;
use std::process::ExitCode;

use tracing::{error, info};

use crate::OracleError;

/// The first difference between two records of a case.
#[derive(Debug, Clone, PartialEq)]
pub enum Mismatch {
    /// The series headers differ.
    Header {
        /// Native header line.
        native: String,
        /// Wasm header line.
        wasm: String,
    },
    /// One record has more lines than the other.
    Length {
        /// Native line count.
        native: usize,
        /// Wasm line count.
        wasm: usize,
    },
    /// A sampled value differs.
    Value {
        /// Sample time of the row, from the native record.
        time: f64,
        /// Column name.
        property: String,
        /// Native bits.
        native: u64,
        /// Wasm bits.
        wasm: u64,
    },
    /// A transcript line differs.
    Line {
        /// One-based line number.
        line: usize,
        /// Native line.
        native: String,
        /// Wasm line.
        wasm: String,
    },
}

impl std::fmt::Display for Mismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Header { native, wasm } => write!(f, "header: native {native:?}, wasm {wasm:?}"),
            Self::Length { native, wasm } => write!(f, "lines: native {native}, wasm {wasm}"),
            Self::Value {
                time,
                property,
                native,
                wasm,
            } => write!(
                f,
                "t={time:?} {property}: native {:?} ({native:#018x}), wasm {:?} ({wasm:#018x})",
                f64::from_bits(*native),
                f64::from_bits(*wasm)
            ),
            Self::Line { line, native, wasm } => {
                write!(f, "line {line}: native {native:?}, wasm {wasm:?}")
            }
        }
    }
}

/// What comparing two records of a case found.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// Series rows or transcript lines compared.
    pub rows: usize,
    /// Values compared: rows times columns, or transcript lines.
    pub values: usize,
    /// The first difference, if any.
    pub mismatch: Option<Mismatch>,
}

/// Compares the records `native` and `wasm` of `case`; `bits` says
/// whether they are series bits rather than transcripts.
pub fn compare_records(
    case: &str,
    bits: bool,
    native: &str,
    wasm: &str,
) -> Result<Outcome, OracleError> {
    if !bits {
        return Ok(compare_lines(native, wasm));
    }
    let native_lines: Vec<&str> = native.lines().collect();
    let wasm_lines: Vec<&str> = wasm.lines().collect();
    let header = native_lines.first().copied().unwrap_or_default();
    let wasm_header = wasm_lines.first().copied().unwrap_or_default();
    let mut outcome = Outcome {
        rows: 0,
        values: 0,
        mismatch: None,
    };
    if header != wasm_header {
        outcome.mismatch = Some(Mismatch::Header {
            native: header.to_owned(),
            wasm: wasm_header.to_owned(),
        });
        return Ok(outcome);
    }
    let columns: Vec<&str> = header.split(',').collect();
    let rows = native_lines.iter().zip(&wasm_lines).skip(1);
    for (line, (native_row, wasm_row)) in rows.enumerate() {
        let native_cells = parse_row(case, line, &columns, native_row)?;
        let wasm_cells = parse_row(case, line, &columns, wasm_row)?;
        let time = native_cells
            .first()
            .copied()
            .map_or(f64::NAN, f64::from_bits);
        for ((&n, &w), property) in native_cells.iter().zip(&wasm_cells).zip(&columns) {
            if n != w {
                outcome.mismatch = Some(Mismatch::Value {
                    time,
                    property: (*property).to_owned(),
                    native: n,
                    wasm: w,
                });
                return Ok(outcome);
            }
        }
        outcome.rows = outcome.rows.wrapping_add(1);
        outcome.values = outcome.values.wrapping_add(native_cells.len());
    }
    if native_lines.len() != wasm_lines.len() {
        outcome.mismatch = Some(Mismatch::Length {
            native: native_lines.len(),
            wasm: wasm_lines.len(),
        });
    }
    Ok(outcome)
}

fn parse_row(
    case: &str,
    row: usize,
    columns: &[&str],
    text: &str,
) -> Result<Vec<u64>, OracleError> {
    let error = |message: String| OracleError::Determinism {
        case: case.to_owned(),
        message: format!("row {row}: {message}"),
    };
    let cells = text
        .split(',')
        .map(|cell| u64::from_str_radix(cell, 16).map_err(|e| error(format!("{cell:?}: {e}"))))
        .collect::<Result<Vec<u64>, _>>()?;
    if cells.len() == columns.len() {
        Ok(cells)
    } else {
        Err(error(format!(
            "{} cells, header has {}",
            cells.len(),
            columns.len()
        )))
    }
}

fn compare_lines(native: &str, wasm: &str) -> Outcome {
    let mut rows = 0usize;
    let mut native_lines = native.lines();
    let mut wasm_lines = wasm.lines();
    loop {
        match (native_lines.next(), wasm_lines.next()) {
            (None, None) => {
                return Outcome {
                    rows,
                    values: rows,
                    mismatch: None,
                };
            }
            (Some(n), Some(w)) if n == w => rows = rows.wrapping_add(1),
            (n, w) => {
                let mismatch = match (n, w) {
                    (Some(n), Some(w)) => Mismatch::Line {
                        line: rows.wrapping_add(1),
                        native: n.to_owned(),
                        wasm: w.to_owned(),
                    },
                    _ => Mismatch::Length {
                        native: rows.wrapping_add(usize::from(n.is_some())),
                        wasm: rows.wrapping_add(usize::from(w.is_some())),
                    },
                };
                return Outcome {
                    rows,
                    values: rows,
                    mismatch: Some(mismatch),
                };
            }
        }
    }
}

/// Compares the records of `cases` in directories `native` and `wasm` and
/// logs the first mismatch of each case. Fails if any case differs.
pub fn compare_dirs_blocking(
    native: &Path,
    wasm: &Path,
    cases: &[String],
) -> Result<ExitCode, OracleError> {
    let mut code = ExitCode::SUCCESS;
    let (mut rows, mut values) = (0usize, 0usize);
    for case in cases {
        let bits_name = format!("{case}.bits");
        let bits = native.join(&bits_name).is_file();
        let name = if bits {
            bits_name
        } else {
            format!("{case}.txt")
        };
        let read = |dir: &Path| {
            let path = dir.join(&name);
            std::fs::read_to_string(&path).map_err(|e| OracleError::io("reading", &path, e))
        };
        let outcome = compare_records(case, bits, &read(native)?, &read(wasm)?)?;
        rows = rows.wrapping_add(outcome.rows);
        values = values.wrapping_add(outcome.values);
        let unit = if bits { "rows" } else { "lines" };
        match &outcome.mismatch {
            None => info!(
                "{case}: bit-identical, {} {unit}, {} values",
                outcome.rows, outcome.values
            ),
            Some(mismatch) => {
                error!("{case}: FIRST MISMATCH {mismatch}");
                code = ExitCode::FAILURE;
            }
        }
    }
    info!(
        "{} cases, {rows} rows and lines, {values} values compared",
        cases.len()
    );
    Ok(code)
}
