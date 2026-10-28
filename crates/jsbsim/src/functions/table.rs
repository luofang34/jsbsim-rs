// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/math/FGTable.{h,cpp} (lookup).
//
//! Lookup tables of any dimension.
//!
//! Data keeps JSBSim's flat layout so lookups index it exactly as C++ does:
//! a 1D table is `[NaN, NaN, k1, v1, k2, v2, ...]`; a 2D table is
//! `[NaN, c1..cn, r1, v11..v1n, r2, ...]`, row-major with the column keys
//! first; an N-dimensional table holds `[NaN, b1..bm]` breakpoints of its
//! outermost (last) key and one (N-1)-dimensional sub-table per breakpoint.

mod load;

use super::PropertyValue;

/// Shape of a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableKind {
    /// One key.
    OneD,
    /// Row and column keys.
    TwoD,
    /// Breakpoints over sub-tables.
    Nested,
}

/// `FGTable`.
#[derive(Debug, Clone)]
pub struct Table {
    kind: TableKind,
    n_rows: usize,
    n_cols: usize,
    n_dims: usize,
    data: Vec<f64>,
    tables: Vec<Table>,
    lookup: Vec<Option<PropertyValue>>,
    internal: bool,
    name: String,
}

impl Table {
    /// Shape.
    #[must_use]
    pub fn kind(&self) -> TableKind {
        self.kind
    }

    /// Number of keys.
    #[must_use]
    pub fn dimensions(&self) -> usize {
        self.n_dims
    }

    /// Whether the owner supplies the keys (`type="internal"`).
    #[must_use]
    pub fn is_internal(&self) -> bool {
        self.internal
    }

    /// Property name of a named, non-internal table.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Lookup properties, by axis (row, column, table, ...).
    #[must_use]
    pub fn lookup_properties(&self) -> &[Option<PropertyValue>] {
        &self.lookup
    }

    /// `GetElement(r, c)`.
    #[must_use]
    pub fn element(&self, r: usize, c: usize) -> f64 {
        match self.kind {
            TableKind::Nested => self.data[r],
            _ => self.data[r * (self.n_cols + 1) + c],
        }
    }

    /// `GetValue(keys)`: interpolates at `keys`, ordered row, column,
    /// table, ...; returns `None` when fewer keys than dimensions are given.
    #[must_use]
    pub fn value(&self, keys: &[f64]) -> Option<f64> {
        if keys.len() < self.n_dims {
            return None;
        }
        Some(match self.kind {
            TableKind::OneD => self.value_1d(keys[0]),
            TableKind::TwoD => self.value_2d(keys[0], keys[1]),
            TableKind::Nested => self.value_nd(keys),
        })
    }

    fn value_1d(&self, key: f64) -> f64 {
        let d = &self.data;
        let n = self.n_rows;
        if key <= d[2] {
            return d[3];
        } else if key >= d[2 * n] {
            return d[2 * n + 1];
        }
        let mut r = 2;
        while d[2 * r] < key {
            r += 1;
        }
        let x0 = d[2 * r - 2];
        let span = d[2 * r] - x0;
        let factor = (key - x0) / span;
        let y0 = d[2 * r - 1];
        factor * (d[2 * r + 1] - y0) + y0
    }

    fn value_2d(&self, row_key: f64, col_key: f64) -> f64 {
        let d = &self.data;
        let w = self.n_cols + 1;
        if self.n_cols == 1 {
            return self.value_1d(row_key);
        }
        let mut c = 2;
        while d[c] < col_key && c < self.n_cols {
            c += 1;
        }
        let x0 = d[c - 1];
        let span = d[c] - x0;
        let c_factor = crate::units::constrain(0.0, (col_key - x0) / span, 1.0);
        if self.n_rows == 1 {
            let y0 = d[w + c - 1];
            return c_factor * (d[w + c] - y0) + y0;
        }
        let mut r = 2;
        while d[r * w] < row_key && r < self.n_rows {
            r += 1;
        }
        let x0 = d[(r - 1) * w];
        let span = d[r * w] - x0;
        let r_factor = crate::units::constrain(0.0, (row_key - x0) / span, 1.0);
        let col1 = r_factor * d[r * w + c - 1] + (1.0 - r_factor) * d[(r - 1) * w + c - 1];
        let col2 = r_factor * d[r * w + c] + (1.0 - r_factor) * d[(r - 1) * w + c];
        c_factor * (col2 - col1) + col1
    }

    fn value_nd(&self, keys: &[f64]) -> f64 {
        let d = &self.data;
        let n = self.n_rows;
        let outer = keys[self.n_dims - 1];
        let sub = |i: usize| self.tables[i].value(keys).unwrap_or(f64::NAN);
        if outer <= d[1] {
            return sub(0);
        } else if outer >= d[n] {
            return sub(n - 1);
        }
        let mut r = 2;
        while d[r] < outer {
            r += 1;
        }
        let x0 = d[r - 1];
        let span = d[r] - x0;
        let factor = (outer - x0) / span;
        let y0 = sub(r - 2);
        factor * (sub(r - 1) - y0) + y0
    }

    /// `GetMinValue()` of a 1D table.
    #[must_use]
    pub fn min_value(&self) -> f64 {
        (1..=self.n_rows).fold(f64::INFINITY, |m, i| m.min(self.data[2 * i + 1]))
    }
}
