// SPDX-License-Identifier: LGPL-2.1-or-later
//! XML probe; twin of `tools/oracle/cpp/probe_xml.cpp`.

use std::fmt::Write as _;
use std::path::PathBuf;

use jsbsim::oracle_support::xml::{Element, parse_document};

use super::ProbeContext;

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n")
}

/// Appends the dump of `element` and its descendants.
pub fn dump(out: &mut String, element: &Element, depth: usize) {
    let indent = " ".repeat(2 * depth);
    writeln!(
        out,
        "{indent}E {} line={}",
        element.name(),
        element.line_number()
    )
    .ok();
    for (key, value) in element.attributes() {
        writeln!(out, "{indent}  @{key}={}", escape(value)).ok();
    }
    for line in element.data_lines() {
        writeln!(out, "{indent}  |{}", escape(line)).ok();
    }
    for child in element.children() {
        dump(out, child, depth + 1);
    }
}

fn path_of(line: &str, context: &ProbeContext) -> Option<PathBuf> {
    let (kind, rel) = line.split_once(' ')?;
    match kind {
        "file" => context.jsbsim_root.as_ref().map(|root| root.join(rel)),
        _ => Some(context.input_dir.join(rel)),
    }
}

/// Dumps every file listed in `input`.
pub fn run(input: &str, context: &ProbeContext) -> String {
    let mut out = String::new();
    for line in input.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        writeln!(out, "== {line}").ok();
        let bytes = path_of(line, context).and_then(|p| std::fs::read(p).ok());
        match bytes {
            None => out.push_str("MISSING\n"),
            Some(bytes) => match parse_document(&bytes, line) {
                Ok(element) => dump(&mut out, &element, 0),
                Err(_) => out.push_str("ERROR\n"),
            },
        }
    }
    out
}
