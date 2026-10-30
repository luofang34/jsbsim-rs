// SPDX-License-Identifier: LGPL-2.1-or-later
//! Function probe; twin of `tools/oracle/cpp/probe_functions.cpp`.

use std::collections::BTreeSet;

use jsbsim::oracle_support::c_numeric::format_g;
use jsbsim::oracle_support::functions::{Evaluator, FnId, FunctionSet};
use jsbsim::oracle_support::properties::{Attribute, PropertyTree};
use jsbsim::oracle_support::xml::{Element, parse_document};

use super::ProbeContext;

#[derive(Default)]
struct Probe {
    set: FunctionSet,
    tree: PropertyTree,
    skipped: BTreeSet<String>,
    functions: Vec<(String, FnId)>,
}

impl Probe {
    fn declare_properties(&mut self, root: &Element) -> Result<(), jsbsim::Error> {
        for p in root.find_elements("property") {
            let value = if p.attribute("value").is_empty() {
                0.0
            } else {
                p.attribute_as_number("value")?
            };
            let name = p.data_line(0);
            if self.tree.has_node(name)? {
                continue;
            }
            let node = self.tree.get_or_create(self.tree.root(), name)?;
            self.tree.set_double(node, value);
        }
        Ok(())
    }

    fn walk(&mut self, el: &Element) -> Result<(), jsbsim::Error> {
        for child in el.children() {
            if child.name() != "function" {
                self.walk(child)?;
                continue;
            }
            let mut name = child.attribute("name").to_owned();
            if child.attribute("type") == "template" {
                self.set.add_template(&mut self.tree, &name, child)?;
                continue;
            }
            if name.is_empty() {
                name = el.attribute("name").to_owned();
            }
            if !self.skipped.contains(&name) {
                if name.is_empty() {
                    name = format!("#{}", self.functions.len());
                }
                let id = self.set.load_function(&mut self.tree, child, "")?;
                self.functions.push((name, id));
            }
        }
        Ok(())
    }

    fn load(&mut self, kind: &str, rel: &str, context: &ProbeContext) -> String {
        let path = match kind {
            "file" => context.jsbsim_root.clone().unwrap_or_default().join(rel),
            _ => context.input_dir.join(rel),
        };
        let Ok(bytes) = std::fs::read(&path) else {
            return " missing".to_owned();
        };
        let loaded = parse_document(&bytes, rel)
            .map_err(jsbsim::Error::from)
            .and_then(|doc| {
                self.declare_properties(&doc)?;
                self.walk(&doc)
            });
        match loaded {
            Ok(()) => format!(" {} functions", self.functions.len()),
            Err(jsbsim::Error::Unsupported { construct, .. }) => {
                format!(" unsupported {construct}")
            }
            Err(e) => {
                tracing::debug!("load-error: {e}");
                " load-error".to_owned()
            }
        }
    }

    fn eval(&mut self, only: &[&str]) -> String {
        let mut out = String::new();
        for (name, id) in &self.functions {
            if !only.is_empty() && !only.contains(&name.as_str()) {
                continue;
            }
            let value = Evaluator::new(&self.set, &mut self.tree)
                .function(*id)
                .map_or_else(|_| "error".to_owned(), |v| format_g(v, 17));
            out.push_str(&format!(" {name} {value}"));
        }
        out
    }

    fn get(&mut self, path: &str) -> String {
        let Ok(Some(node)) = self.tree.get(path) else {
            return " null".to_owned();
        };
        let value = Evaluator::new(&self.set, &mut self.tree)
            .read_node(node)
            .map_or_else(|_| "error".to_owned(), |v| format_g(v, 17));
        let flag = |b: bool| if b { "1" } else { "0" };
        format!(
            " value {value} tied {} write {}",
            flag(self.tree.is_tied(node)),
            flag(self.tree.attribute(node, Attribute::WRITE))
        )
    }

    fn run(&mut self, words: &[&str], context: &ProbeContext) -> String {
        let arg = |i: usize| words.get(i).copied().unwrap_or_default();
        match arg(0) {
            "skip" => {
                self.skipped.insert(arg(1).to_owned());
                " ok".to_owned()
            }
            "load" => self.load(arg(1), arg(2), context),
            "set" => {
                if let Ok(node) = self.tree.get_or_create(self.tree.root(), arg(1)) {
                    self.tree
                        .set_double(node, arg(2).parse().unwrap_or_default());
                }
                " ok".to_owned()
            }
            "eval" => self.eval(words.get(1..).unwrap_or_default()),
            "get" => self.get(arg(1)),
            "cache" => {
                let ids: Vec<FnId> = self
                    .functions
                    .iter()
                    .filter(|(n, _)| n == arg(1))
                    .map(|(_, id)| *id)
                    .collect();
                for id in ids {
                    self.set.cache_value(id, arg(2) != "0", &mut self.tree).ok();
                }
                " ok".to_owned()
            }
            "reset" => {
                *self = Self::default();
                " ok".to_owned()
            }
            _ => " unknown".to_owned(),
        }
    }
}

/// Executes the operations in `input`, one per line.
pub fn run(input: &str, context: &ProbeContext) -> String {
    let mut probe = Probe::default();
    let mut out = String::new();
    for line in input.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let result = probe.run(&words, context);
        out.push_str(&format!("{line} =>{result}\n"));
    }
    out
}
