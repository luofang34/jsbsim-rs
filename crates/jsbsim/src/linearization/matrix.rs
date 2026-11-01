// SPDX-License-Identifier: LGPL-2.1-or-later
//! The dense matrices of a [`Linearization`](super::Linearization).

/// A dense matrix of `f64`, stored by rows in one allocation.
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix {
    nrows: usize,
    ncols: usize,
    /// `nrows * ncols` entries, rows first.
    data: Vec<f64>,
}

impl Matrix {
    /// A `nrows` by `ncols` matrix over `data`, which holds `nrows * ncols`
    /// entries, rows first.
    pub(super) fn from_row_major(nrows: usize, ncols: usize, data: Vec<f64>) -> Self {
        Self { nrows, ncols, data }
    }

    /// The number of rows.
    #[must_use]
    pub fn nrows(&self) -> usize {
        self.nrows
    }

    /// The number of columns.
    #[must_use]
    pub fn ncols(&self) -> usize {
        self.ncols
    }

    /// The number of rows and the number of columns.
    #[must_use]
    pub fn shape(&self) -> (usize, usize) {
        (self.nrows, self.ncols)
    }

    /// Every entry, row after row.
    #[must_use]
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }

    /// Row `r`, or `None` past the last row.
    #[must_use]
    pub fn row(&self, r: usize) -> Option<&[f64]> {
        if r >= self.nrows {
            return None;
        }
        let start = r * self.ncols;
        self.data.get(start..start + self.ncols)
    }

    /// The rows, in order.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &[f64]> {
        (0..self.nrows).map(|r| self.row(r).unwrap_or_default())
    }

    /// The entry at `row` and `col`, or `None` outside the matrix.
    #[must_use]
    pub fn get(&self, row: usize, col: usize) -> Option<f64> {
        self.row(row)?.get(col).copied()
    }
}
