// SPDX-License-Identifier: LGPL-2.1-or-later
//! Table probe; twin of `tools/oracle/cpp/probe_table.cpp`.

use jsbsim::c_numeric::format_g;
use jsbsim::functions::{FunctionSet, Table};
use jsbsim::properties::PropertyTree;
use jsbsim::xml::parse_document;

use super::ProbeContext;

fn load(prefix: &str, path: &str, context: &ProbeContext) -> Result<Option<Table>, jsbsim::Error> {
    let Ok(bytes) = std::fs::read(context.input_dir.join(path)) else {
        return Ok(None);
    };
    let doc = parse_document(&bytes, path)?;
    let Some(el) = doc.find_element("table") else {
        return Ok(None);
    };
    let mut set = FunctionSet::new();
    let mut tree = PropertyTree::new();
    let prefix = if prefix == "-" { "" } else { prefix };
    let id = set.load_table(&mut tree, el, prefix)?;
    Ok(Some(set.table(id)?.clone()))
}

/// Executes the operations in `input`, one per line.
pub fn run(input: &str, context: &ProbeContext) -> String {
    let mut table: Option<Table> = None;
    let mut out = String::new();
    for line in input.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let arg = |i: usize| words.get(i).copied().unwrap_or_default();
        let result = match arg(0) {
            "table" => match load(arg(1), arg(2), context) {
                Ok(Some(t)) => {
                    table = Some(t);
                    "ok".to_owned()
                }
                Ok(None) => {
                    table = None;
                    "missing".to_owned()
                }
                Err(_) => {
                    table = None;
                    "load-error".to_owned()
                }
            },
            "lookup" => {
                let keys: Vec<f64> = words
                    .iter()
                    .skip(1)
                    .filter_map(|w| w.parse().ok())
                    .collect();
                match &table {
                    Some(t) => t
                        .value(&keys)
                        .map_or_else(|| "error".to_owned(), |v| format_g(v, 17)),
                    None => "no-table".to_owned(),
                }
            }
            "element" => match &table {
                Some(t) => format_g(
                    t.element(arg(1).parse().unwrap_or(0), arg(2).parse().unwrap_or(0)),
                    17,
                ),
                None => "no-table".to_owned(),
            },
            _ => "unknown".to_owned(),
        };
        out.push_str(&format!("{line} => {result}\n"));
    }
    out
}
