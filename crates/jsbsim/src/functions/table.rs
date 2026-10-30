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

use std::cell::Cell;

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
    tables: Vec<Self>,
    lookup: Vec<Option<PropertyValue>>,
    internal: bool,
    name: String,
    /// One slot per dimension, where a nested lookup gathers its keys
    /// before interpolating, so the lookup needs no buffer of its own. The
    /// evaluator reaches tables through a shared borrow of the function
    /// set, hence `Cell`; it is `Send`, so an FDM still moves between
    /// threads, and only `Sync` sharing is ruled out.
    keys: Box<[Cell<f64>]>,
}

impl Table {
    /// `FGTable(NRows)` filled with `<<`: a 1D table of `(key, value)` rows
    /// built in code rather than loaded.
    #[must_use]
    pub fn one_d(rows: &[(f64, f64)]) -> Self {
        let mut data = vec![f64::NAN, f64::NAN];
        for &(key, value) in rows {
            data.push(key);
            data.push(value);
        }
        Self {
            kind: TableKind::OneD,
            n_rows: rows.len(),
            n_cols: 1,
            n_dims: 1,
            data,
            tables: Vec::new(),
            lookup: vec![None],
            internal: false,
            name: String::new(),
            keys: key_slots(1),
        }
    }

    /// `FGTable(NRows, NCols)` filled with `<<`: the column keys, then each
    /// row's key and values, built in code rather than loaded. A row with
    /// fewer values than columns is padded with NaN.
    #[must_use]
    pub fn two_d(col_keys: &[f64], rows: &[(f64, &[f64])]) -> Self {
        let n_cols = col_keys.len();
        let mut data = Vec::with_capacity((rows.len() + 1) * (n_cols + 1));
        data.push(f64::NAN);
        data.extend_from_slice(col_keys);
        for &(key, values) in rows {
            data.push(key);
            data.extend((0..n_cols).map(|c| values.get(c).copied().unwrap_or(f64::NAN)));
        }
        Self {
            kind: TableKind::TwoD,
            n_rows: rows.len(),
            n_cols,
            n_dims: 2,
            data,
            tables: Vec::new(),
            lookup: vec![None, None],
            internal: false,
            name: String::new(),
            keys: key_slots(2),
        }
    }

    /// `GetNumRows()`.
    #[must_use]
    pub fn num_rows(&self) -> usize {
        self.n_rows
    }

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

    /// `GetElement(r, c)`; NaN outside the data, where JSBSim reads past
    /// the end of its array.
    #[must_use]
    pub fn element(&self, r: usize, c: usize) -> f64 {
        let i = match self.kind {
            TableKind::Nested => r,
            _ => r * (self.n_cols + 1) + c,
        };
        self.data.get(i).copied().unwrap_or(f64::NAN)
    }

    /// `GetValue(keys)`: interpolates at `keys`, ordered row, column,
    /// table, ...; returns `None` when fewer keys than dimensions are given.
    #[must_use]
    pub fn value(&self, keys: &[f64]) -> Option<f64> {
        if keys.len() < self.n_dims {
            return None;
        }
        Some(self.value_by(&|i| keys.get(i).copied().unwrap_or(f64::NAN)))
    }

    /// Stores key `axis` for [`value_of_keys`](Self::value_of_keys).
    pub(crate) fn set_key(&self, axis: usize, key: f64) {
        if let Some(slot) = self.keys.get(axis) {
            slot.set(key);
        }
    }

    /// `GetValue()` at the keys last stored with `set_key`.
    pub(crate) fn value_of_keys(&self) -> f64 {
        self.value_by(&|i| self.keys.get(i).map_or(f64::NAN, Cell::get))
    }

    fn value_by(&self, key: &impl Fn(usize) -> f64) -> f64 {
        match self.kind {
            TableKind::OneD => self.value_1d(key(0)),
            TableKind::TwoD => self.value_2d(key(0), key(1)),
            TableKind::Nested => self.value_nd(key),
        }
    }

    /// The `(key, value)` rows of a 1D table.
    fn rows_1d(&self) -> &[[f64; 2]] {
        let pairs = self.data.get(2..).unwrap_or_default().as_chunks().0;
        pairs.get(..self.n_rows).unwrap_or(pairs)
    }

    /// `GetValue(key)` of a 1D table. NaN where JSBSim would read past its
    /// data: a NaN key, which no breakpoint bounds, in a one-row table.
    #[must_use]
    pub fn value_1d(&self, key: f64) -> f64 {
        let rows = self.rows_1d();
        let (Some(&[k1, v1]), Some(&[kn, vn])) = (rows.first(), rows.last()) else {
            return f64::NAN;
        };
        if key <= k1 {
            return v1;
        } else if key >= kn {
            return vn;
        }
        // JSBSim's linear search from the second row: `i + 1` is the first
        // row whose key is not below `key`.
        let below = |&[k, _]: &[f64; 2]| k < key;
        let i = rows
            .get(1..)
            .map_or(0, |r| r.iter().take_while(|row| below(row)).count());
        let Some(&[[x0, y0], [x1, y1]]) = rows.get(i..=i + 1) else {
            return f64::NAN;
        };
        let span = x1 - x0;
        let factor = (key - x0) / span;
        factor * (y1 - y0) + y0
    }

    /// `GetValue(rowKey, colKey)` of a 2D table.
    #[must_use]
    pub fn value_2d(&self, row_key: f64, col_key: f64) -> f64 {
        if self.n_cols == 1 {
            return self.value_1d(row_key);
        }
        self.lookup_2d(row_key, col_key).unwrap_or(f64::NAN)
    }

    /// [`value_2d`](Self::value_2d) past the 1-column case; `None` only
    /// for data shorter than its shape, which the loader rejects.
    fn lookup_2d(&self, row_key: f64, col_key: f64) -> Option<f64> {
        let (n_rows, n_cols) = (self.n_rows, self.n_cols);
        let w = n_cols + 1;
        let row = |r: usize| self.data.get(r * w..(r + 1) * w);
        // JSBSim's searches start at 2 and stop at the last row or column.
        let c = 2 + row(0)?
            .get(2..n_cols)
            .map_or(0, |keys| keys.iter().take_while(|&&k| k < col_key).count());
        let &[x0, x1] = row(0)?.get(c - 1..=c)? else {
            return None;
        };
        let span = x1 - x0;
        let c_factor = crate::units::constrain(0.0, (col_key - x0) / span, 1.0);
        if n_rows == 1 {
            let (_, y0, y1) = row_at(row(1)?, c)?;
            return Some(c_factor * (y1 - y0) + y0);
        }
        let r = 2 + self
            .data
            .chunks_exact(w)
            .take(n_rows)
            .skip(2)
            .take_while(|row| row.first().is_some_and(|&k| k < row_key))
            .count();
        let (x0, lo0, lo1) = row_at(row(r - 1)?, c)?;
        let (x1, hi0, hi1) = row_at(row(r)?, c)?;
        let span = x1 - x0;
        let r_factor = crate::units::constrain(0.0, (row_key - x0) / span, 1.0);
        let col1 = r_factor * hi0 + (1.0 - r_factor) * lo0;
        let col2 = r_factor * hi1 + (1.0 - r_factor) * lo1;
        Some(c_factor * (col2 - col1) + col1)
    }

    /// `GetValue(keys)` of a nested table. NaN where JSBSim would read past
    /// its data: a NaN outer key with a single breakpoint.
    fn value_nd(&self, key: &impl Fn(usize) -> f64) -> f64 {
        let outer = self.n_dims.checked_sub(1).map_or(f64::NAN, key);
        let sub = |i: usize| self.tables.get(i).map_or(f64::NAN, |t| t.value_by(key));
        let breaks = self.data.get(1..=self.n_rows).unwrap_or_default();
        let (Some(&first), Some(&last)) = (breaks.first(), breaks.last()) else {
            return f64::NAN;
        };
        if outer <= first {
            return sub(0);
        } else if outer >= last {
            return sub(breaks.len() - 1);
        }
        // JSBSim's linear search from the second breakpoint: `p + 1` is the
        // first breakpoint not below `outer`, `p` the sub-table below it.
        let p = breaks
            .get(1..)
            .map_or(0, |b| b.iter().take_while(|&&b| b < outer).count());
        let Some(&[x0, x1]) = breaks.get(p..=p + 1) else {
            return f64::NAN;
        };
        let span = x1 - x0;
        let factor = (outer - x0) / span;
        let y0 = sub(p);
        factor * (sub(p + 1) - y0) + y0
    }

    /// `GetMinValue()` of a 1D table.
    #[must_use]
    pub fn min_value(&self) -> f64 {
        self.rows_1d()
            .iter()
            .fold(f64::INFINITY, |m, &[_, v]| m.min(v))
    }
}

/// The key of a 2D table row and its values in columns `c - 1` and `c`.
fn row_at(row: &[f64], c: usize) -> Option<(f64, f64, f64)> {
    match (row.first(), row.get(c - 1..=c)) {
        (Some(&key), Some(&[a, b])) => Some((key, a, b)),
        _ => None,
    }
}

/// Key slots for a table of `n_dims` dimensions.
fn key_slots(n_dims: usize) -> Box<[Cell<f64>]> {
    (0..n_dims).map(|_| Cell::new(f64::NAN)).collect()
}

crate::snapshot::visit_fields! {
    Table { tables, keys }
    each { lookup }
    skip { kind, n_rows, n_cols, n_dims, data, internal, name }
}
