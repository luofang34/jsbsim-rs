// SPDX-License-Identifier: LGPL-2.1-or-later
//! Property-tree probe; twin of `tools/oracle/cpp/probe_properties.cpp`.

use jsbsim::oracle_support::c_numeric::format_g;
use jsbsim::oracle_support::properties::{
    Attribute, NodeId, PathError, PropertyTree, Value, ValueType,
};

fn attribute(name: &str) -> Attribute {
    match name {
        "READ" => Attribute::READ,
        "WRITE" => Attribute::WRITE,
        "ARCHIVE" => Attribute::ARCHIVE,
        "REMOVED" => Attribute::REMOVED,
        "TRACE_READ" => Attribute::TRACE_READ,
        "TRACE_WRITE" => Attribute::TRACE_WRITE,
        "USERARCHIVE" => Attribute::USERARCHIVE,
        _ => Attribute::PRESERVE,
    }
}

fn type_name(t: ValueType) -> &'static str {
    match t {
        ValueType::None => "NONE",
        ValueType::Bool => "BOOL",
        ValueType::Int => "INT",
        ValueType::Long => "LONG",
        ValueType::Float => "FLOAT",
        ValueType::Double => "DOUBLE",
        ValueType::String => "STRING",
        ValueType::Unspecified => "UNSPECIFIED",
    }
}

fn flag(b: bool) -> String {
    if b { "1" } else { "0" }.to_owned()
}

fn get(tree: &PropertyTree, id: NodeId, kind: &str) -> String {
    match kind {
        "bool" => flag(tree.get_bool(id)),
        "int" => tree.get_int(id).to_string(),
        "long" => tree.get_long(id).to_string(),
        "float" => format_g(f64::from(tree.get_float(id)), 17),
        "double" => format_g(tree.get_double(id), 17),
        _ => format!("\"{}\"", tree.get_string(id)),
    }
}

fn set(tree: &mut PropertyTree, id: NodeId, kind: &str, text: &str) -> String {
    let done = match kind {
        "bool" => tree.set_bool(id, text == "1"),
        "int" => tree.set_int(id, text.parse().unwrap_or_default()),
        "long" => tree.set_long(id, text.parse().unwrap_or_default()),
        "float" => tree.set_float(id, text.parse().unwrap_or_default()),
        "double" => tree.set_double(id, text.parse().unwrap_or_default()),
        "unspecified" => tree.set_unspecified(id, text),
        _ => tree.set_string(id, text),
    };
    flag(done)
}

struct Probe {
    tree: PropertyTree,
}

impl Probe {
    fn require(&mut self, path: &str) -> Result<NodeId, PathError> {
        self.tree.get_or_create(self.tree.root(), path)
    }

    fn fqn(&self, node: Option<NodeId>) -> String {
        node.map_or_else(
            || "null".to_owned(),
            |id| self.tree.fully_qualified_name(id),
        )
    }

    fn run(&mut self, op: &str, args: &[&str], rest: &str) -> Result<String, PathError> {
        let a = args.first().copied().unwrap_or_default();
        let b = args.get(1).copied().unwrap_or_default();
        let index = || b.parse::<i32>().unwrap_or_default();
        let root = self.tree.root();
        Ok(match op {
            "find" => self.fqn(self.tree.find(root, a)?),
            "create" => {
                let id = self.require(a)?;
                self.fqn(Some(id))
            }
            "findi" => self.fqn(self.tree.find_indexed(root, a, Some(index()))?),
            "createi" => {
                let id = self.tree.get_or_create_indexed(root, a, Some(index()))?;
                self.fqn(Some(id))
            }
            "has" => flag(self.tree.has_node(a)?),
            _ => return self.run_value_op(op, args, rest),
        })
    }

    fn run_value_op(&mut self, op: &str, args: &[&str], rest: &str) -> Result<String, PathError> {
        let a = args.first().copied().unwrap_or_default();
        let b = args.get(1).copied().unwrap_or_default();
        Ok(match op {
            "get" => match self.tree.get(b)? {
                Some(id) => get(&self.tree, id, a),
                None => "null".to_owned(),
            },
            "set" => {
                let id = self.require(b)?;
                set(&mut self.tree, id, a, rest)
            }
            "type" => {
                let id = self.require(a)?;
                type_name(self.tree.value_type(id)).to_owned()
            }
            "path" => {
                let id = self.require(a)?;
                self.tree.path(id, false)
            }
            "alias" => {
                let id = self.require(a)?;
                let target = self.require(b)?;
                flag(self.tree.alias(id, target))
            }
            "alias-rel" => {
                let id = self.require(a)?;
                flag(self.tree.alias_path(id, b)?)
            }
            "unalias" => {
                let id = self.require(a)?;
                flag(self.tree.unalias(id))
            }
            "attr" => {
                let id = self.require(a)?;
                let on = args.get(2).is_some_and(|v| *v != "0");
                self.tree.set_attribute(id, attribute(b), on);
                "ok".to_owned()
            }
            "getattr" => {
                let id = self.require(a)?;
                flag(self.tree.attribute(id, attribute(b)))
            }
            "tie-rw" | "tie-ro" => {
                let id = self.require(a)?;
                let (initial, writable) = if op == "tie-rw" {
                    (0.0, true)
                } else {
                    (42.0, false)
                };
                self.tree.tie(id, Value::Double(initial), writable);
                flag(self.tree.is_tied(id))
            }
            "untie" => {
                if let Some(id) = self.tree.get(a)? {
                    self.tree.untie(id);
                }
                let id = self.require(a)?;
                flag(self.tree.is_tied(id))
            }
            "mkname" => format!(
                "\"{}\"",
                jsbsim::oracle_support::properties::mk_property_name(rest, a == "1")
            ),
            "relname" => {
                let id = self.require(a)?;
                self.tree.relative_name(id, b)
            }
            other => format!("unknown op {other}"),
        })
    }
}

/// Executes the operations in `input`, one per line.
pub fn run(input: &str) -> String {
    let mut probe = Probe {
        tree: PropertyTree::new(),
    };
    let mut out = String::new();
    for line in input.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace();
        let op = words.next().unwrap_or_default();
        let args: Vec<&str> = words.collect();
        let rest = rest_after(line, if op == "set" { 3 } else { 2 });
        let result = probe
            .run(op, &args, rest)
            .unwrap_or_else(|e| format!("error: {}", e.message));
        out.push_str(&format!("{line} => {result}\n"));
    }
    out
}

/// The text after the first `words` whitespace-separated words, with
/// leading whitespace removed (`getline(args >> std::ws, rest)`).
fn rest_after(line: &str, words: usize) -> &str {
    let mut rest = line;
    for _ in 0..words {
        rest = rest.trim_start();
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        rest = &rest[end..];
    }
    rest.trim_start()
}
