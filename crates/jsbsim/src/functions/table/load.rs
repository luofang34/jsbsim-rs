// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/math/FGTable.cpp (construction from XML).
//
//! Building a [`Table`] from `<table>` / `<tableData>` elements.

use super::{Table, TableKind};
use crate::c_numeric::strtod_prefix;
use crate::functions::{FunctionError, PropertyValue, substitute_hash};
use crate::properties::PropertyTree;
use crate::xml::Element;
use crate::{Error, LoadOptions};

fn table_error(el: &Element, message: impl Into<String>) -> Error {
    FunctionError::Table {
        at: el.location(),
        message: message.into(),
    }
    .into()
}

/// `FindNumColumns`: whitespace-separated fields on a line.
fn num_columns(line: &str) -> usize {
    line.split([' ', '\t']).filter(|f| !f.is_empty()).count()
}

/// `ParseLookupAxis`: 0-based axis of a `lookup` attribute.
fn parse_lookup_axis(text: &str) -> Option<usize> {
    match text {
        "" | "row" | "axis1" => Some(0),
        "column" | "axis2" => Some(1),
        "table" | "axis3" => Some(2),
        _ => {
            let n: usize = text.strip_prefix("axis")?.parse().ok()?;
            n.checked_sub(1)
        }
    }
}

/// `LookupAxisName`.
fn lookup_axis_name(axis: usize) -> String {
    match axis {
        0 => "row".to_owned(),
        1 => "column".to_owned(),
        2 => "table".to_owned(),
        n => format!("axis{}", n + 1),
    }
}

/// `InferLeafDimension`: 1 or 2 from the column counts of the data lines.
fn leaf_dimension(data: &Element) -> Result<usize, Error> {
    let lines = data.data_lines();
    let first = lines
        .first()
        .ok_or_else(|| table_error(data, "<tableData> is empty."))?;
    let first_cols = num_columns(first);
    let check_all = |cols: usize| {
        for (i, line) in lines.iter().enumerate().skip(1) {
            if num_columns(line) != cols {
                let line_no = data.line_number() as usize + i;
                return Err(table_error(
                    data,
                    format!("Invalid number of columns in line {line_no}"),
                ));
            }
        }
        Ok(())
    };
    if lines.len() == 1 {
        return if first_cols == 2 {
            Ok(1)
        } else {
            Err(table_error(data, "Too many columns for a 1D table"))
        };
    }
    let second_cols = num_columns(&lines[1]);
    if second_cols == first_cols + 1 {
        check_all(second_cols)?;
        return Ok(2);
    }
    if first_cols != 2 {
        return Err(table_error(data, "Too many columns for a 1D table"));
    }
    check_all(2)?;
    Ok(1)
}

/// `AppendNumericData` followed by `operator<<(istream&)`: numbers are read
/// until the first token that is not one, as an input stream does.
fn numeric_data(data: &Element) -> Result<Vec<f64>, Error> {
    let mut text = String::new();
    for (i, line) in data.data_lines().iter().enumerate() {
        if line.chars().any(|c| !"0123456789.-+eE \t\n".contains(c)) {
            let line_no = data.line_number() as usize + i + 1;
            return Err(table_error(
                data,
                format!("Illegal character found in line {line_no}: {line}"),
            ));
        }
        text.push_str(line);
        text.push(' ');
    }
    let mut values = Vec::new();
    let mut rest = text.as_str();
    loop {
        let (value, consumed) = strtod_prefix(rest);
        if consumed == 0 {
            return Ok(values);
        }
        values.push(value);
        rest = &rest[consumed..];
    }
}

impl Table {
    /// `FGTable(pm, el, prefix)`. Binding a named table to its property is
    /// left to the caller.
    pub fn load(
        el: &Element,
        tree: &PropertyTree,
        prefix: &str,
        options: LoadOptions,
    ) -> Result<Self, Error> {
        let mut table = Self {
            kind: TableKind::OneD,
            n_rows: 0,
            n_cols: 0,
            n_dims: 0,
            data: Vec::new(),
            tables: Vec::new(),
            lookup: Vec::new(),
            internal: false,
            name: el.attribute("name").to_owned(),
        };
        match el.attribute("type") {
            "internal" => table.internal = true,
            "" => {}
            other => {
                return Err(table_error(
                    el,
                    format!("  An unknown table type attribute is listed: {other}"),
                ));
            }
        }
        let declared = table.load_axes(el, tree, prefix, options)?;
        table.load_data(el, tree, options)?;
        if declared != 0 && declared != table.n_dims {
            return Err(table_error(
                el,
                format!(
                    "{declared} lookup axes were declared, but the tableData nesting implies a {}D table.",
                    table.n_dims
                ),
            ));
        }
        table.check_monotonic(el)?;
        table.check_size(el)?;
        table
            .lookup
            .resize(table.n_dims.max(table.lookup.len()), None);
        Ok(table)
    }

    fn load_axes(
        &mut self,
        el: &Element,
        tree: &PropertyTree,
        prefix: &str,
        options: LoadOptions,
    ) -> Result<usize, Error> {
        let mut declared = 0;
        let mut axes = el.find_elements("independentVar").peekable();
        if axes.peek().is_none() {
            if !self.internal && el.name() != "tableData" {
                return Err(table_error(
                    el,
                    "No independentVars found, and table is not marked as internal, nor is it a nested sub-table.",
                ));
            }
            return Ok(0);
        }
        if self.internal {
            let message = "This table specifies both 'internal' call type and specific lookup properties via the 'independentVar' element. The 'internal' attribute will be ignored.";
            if options.strict {
                return Err(FunctionError::Strict {
                    at: el.location(),
                    message: message.to_owned(),
                }
                .into());
            }
            tracing::error!("{}: {message}", el.location());
            self.internal = false;
        }
        for axis_el in axes {
            let name = substitute_hash(axis_el.data_line(0), prefix);
            let value = PropertyValue::new(&name, tree, axis_el.location())?;
            let lookup = axis_el.attribute("lookup");
            let axis = parse_lookup_axis(lookup).ok_or_else(|| {
                table_error(
                    axis_el,
                    format!("lookup axis specification not understood: {lookup}"),
                )
            })?;
            if self.lookup.get(axis).is_some_and(Option::is_some) {
                return Err(table_error(
                    axis_el,
                    format!("duplicate lookup axis \"{}\"", lookup_axis_name(axis)),
                ));
            }
            if self.lookup.len() <= axis {
                self.lookup.resize(axis + 1, None);
            }
            self.lookup[axis] = Some(value);
            declared = declared.max(axis + 1);
        }
        for axis in 0..declared {
            if self.lookup[axis].is_none() {
                return Err(table_error(
                    el,
                    format!("missing lookup axis \"{}\"", lookup_axis_name(axis)),
                ));
            }
        }
        Ok(declared)
    }

    fn load_data(
        &mut self,
        el: &Element,
        tree: &PropertyTree,
        options: LoadOptions,
    ) -> Result<(), Error> {
        let children = el.num_elements("tableData");
        let leaf = if el.name() == "tableData" && children == 0 {
            Some(el)
        } else if el.name() != "tableData" && children == 1 {
            el.find_element("tableData")
        } else if children > 1 {
            return self.load_nested(el, tree, options);
        } else {
            return Err(table_error(el, "<tableData> elements are missing"));
        };
        let Some(leaf) = leaf else {
            return Err(table_error(el, "<tableData> elements are missing"));
        };
        let values = numeric_data(leaf)?;
        self.n_dims = leaf_dimension(leaf)?;
        if self.n_dims == 1 {
            self.n_rows = leaf.data_lines().len();
            self.n_cols = 1;
            self.kind = TableKind::OneD;
            self.data = vec![f64::NAN, f64::NAN];
        } else {
            self.n_rows = leaf.data_lines().len() - 1;
            self.n_cols = num_columns(leaf.data_line(0));
            self.kind = TableKind::TwoD;
            self.data = vec![f64::NAN];
        }
        self.data.extend(values);
        Ok(())
    }

    fn load_nested(
        &mut self,
        el: &Element,
        tree: &PropertyTree,
        options: LoadOptions,
    ) -> Result<(), Error> {
        self.kind = TableKind::Nested;
        self.data = vec![f64::NAN];
        self.n_cols = 1;
        for child in el.find_elements("tableData") {
            if child.attribute("breakPoint").is_empty() {
                return Err(table_error(child, "missing breakPoint on <tableData>"));
            }
            if !child.attribute("name").is_empty() {
                return Err(table_error(
                    child,
                    "a named <tableData> sub-table has no lookup properties to bind",
                ));
            }
            let sub = Table::load(child, tree, "", options)?;
            if self.n_dims == 0 {
                self.n_dims = sub.n_dims + 1;
                if self.n_dims < 3 {
                    return Err(table_error(
                        child,
                        "nested tables must contain at least 2D subtables",
                    ));
                }
            } else if sub.n_dims + 1 != self.n_dims {
                return Err(table_error(
                    child,
                    "inconsistent sub-table dimensionality in nested table",
                ));
            }
            self.data.push(child.attribute_as_number("breakPoint")?);
            self.tables.push(sub);
        }
        self.n_rows = self.tables.len();
        Ok(())
    }

    fn check_monotonic(&self, el: &Element) -> Result<(), Error> {
        let d = &self.data;
        let named = |what: &str, i: usize, a: f64, b: f64| {
            let suffix = if self.name.is_empty() {
                String::new()
            } else {
                format!(" of table in {}", self.name)
            };
            table_error(
                el,
                format!(
                    "{what} lookup is not monotonically increasing in {what} {i}{suffix}: {a}<={b}"
                ),
            )
        };
        match self.kind {
            TableKind::Nested => {
                for b in 2..=self.tables.len() {
                    if d[b] <= d[b - 1] {
                        return Err(named("breakpoint", b, d[b], d[b - 1]));
                    }
                }
                return Ok(());
            }
            TableKind::TwoD => {
                for c in 2..=self.n_cols {
                    if d.get(c).zip(d.get(c - 1)).is_some_and(|(x, y)| x <= y) {
                        return Err(named("column", c, d[c], d[c - 1]));
                    }
                }
            }
            TableKind::OneD => {}
        }
        let w = self.n_cols + 1;
        for r in 2..=self.n_rows {
            if let (Some(&x), Some(&y)) = (d.get(r * w), d.get((r - 1) * w))
                && x <= y
            {
                return Err(named("row", r, x, y));
            }
        }
        Ok(())
    }

    fn check_size(&self, el: &Element) -> Result<(), Error> {
        let (expected, actual, ok) = match self.kind {
            TableKind::OneD => (
                2 * self.n_rows,
                self.data.len().saturating_sub(2),
                self.data.len() == 2 * self.n_rows + 2,
            ),
            TableKind::TwoD => {
                let full = (self.n_rows + 1) * (self.n_cols + 1);
                (
                    full - 1,
                    self.data.len().saturating_sub(1),
                    self.data.len() == full,
                )
            }
            TableKind::Nested => (
                self.n_rows,
                self.data.len().saturating_sub(1),
                self.data.len() == self.n_rows + 1,
            ),
        };
        if ok {
            return Ok(());
        }
        let in_name = if self.name.is_empty() {
            String::new()
        } else {
            format!(" in table {}", self.name)
        };
        Err(table_error(
            el,
            format!(
                "Missing data{in_name}: Expecting {expected} elements while {actual} elements were provided."
            ),
        ))
    }
}
