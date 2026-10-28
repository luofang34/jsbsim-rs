// SPDX-License-Identifier: LGPL-2.1-or-later
// FGXMLParse: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/input_output/FGXMLParse.cpp and the split()
// helper of string_utilities.cpp.
//
//! Building an [`Element`] tree from XML bytes.
//!
//! JSBSim parses with expat without namespace processing and collects
//! character data in a buffer that is flushed into the current element at
//! every start and end tag: split at newlines, each piece trimmed, blank
//! pieces dropped. Comments and processing instructions do not flush the
//! buffer. Attribute names keep their prefixes, and namespace declarations
//! are attributes too.

use std::rc::Rc;

use roxmltree::{Document, Node, ParsingOptions};

use super::{Element, XmlError};
use crate::c_numeric::is_c_space;

/// Parses a document; `file` names it in errors and element locations.
pub fn parse_document(bytes: &[u8], file: &str) -> Result<Element, XmlError> {
    let text = decode(bytes, file)?;
    let options = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    let doc = Document::parse_with_options(&text, options).map_err(|e| {
        let pos = e.pos();
        XmlError::Malformed {
            file: file.to_owned(),
            line: pos.row,
            column: pos.col,
            message: e.to_string(),
        }
    })?;
    let file: Rc<str> = Rc::from(file);
    let mut root = element_shell(&doc, doc.root_element(), &file);
    fill(&doc, doc.root_element(), &mut root, &file);
    Ok(root)
}

/// Converts the bytes to UTF-8 according to the XML declaration, as expat
/// does for the encodings it supports natively.
fn decode(bytes: &[u8], file: &str) -> Result<String, XmlError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let encoding = declared_encoding(bytes).unwrap_or_else(|| "UTF-8".to_owned());
    let error = || XmlError::Encoding {
        file: file.to_owned(),
        encoding: encoding.clone(),
    };
    match encoding.to_ascii_uppercase().as_str() {
        "UTF-8" => String::from_utf8(bytes.to_vec()).map_err(|_| error()),
        "US-ASCII" if bytes.is_ascii() => String::from_utf8(bytes.to_vec()).map_err(|_| error()),
        "ISO-8859-1" => Ok(bytes.iter().map(|&b| char::from(b)).collect()),
        _ => Err(error()),
    }
}

fn declared_encoding(bytes: &[u8]) -> Option<String> {
    let head = bytes.strip_prefix(b"<?xml")?;
    let end = head.windows(2).position(|w| w == b"?>")?;
    let decl = std::str::from_utf8(&head[..end]).ok()?;
    let at = decl.find("encoding")?;
    let rest = decl[at + "encoding".len()..]
        .trim_start()
        .strip_prefix('=')?
        .trim_start();
    let quote = rest.chars().next()?;
    let value = rest[1..].split(quote).next()?;
    Some(value.to_owned())
}

/// Name as expat reports it without namespace processing: `prefix:local`.
fn raw_name(node: Node<'_, '_>, local: &str, namespace: Option<&str>) -> String {
    match namespace.and_then(|uri| node.lookup_prefix(uri)) {
        Some(prefix) if !prefix.is_empty() => format!("{prefix}:{local}"),
        _ => local.to_owned(),
    }
}

fn element_shell(doc: &Document<'_>, node: Node<'_, '_>, file: &Rc<str>) -> Element {
    let tag = node.tag_name();
    let line = doc.text_pos_at(node.range().start).row;
    let mut element = Element::new(
        raw_name(node, tag.name(), tag.namespace()),
        file.clone(),
        line,
    );
    for attr in node.attributes() {
        element.set_attribute(raw_name(node, attr.name(), attr.namespace()), attr.value());
    }
    let inherited: Vec<_> = node
        .parent_element()
        .map(|p| p.namespaces().map(|ns| (ns.name(), ns.uri())).collect())
        .unwrap_or_default();
    for ns in node.namespaces() {
        let declared_here = !inherited.contains(&(ns.name(), ns.uri()));
        if declared_here && ns.name() != Some("xml") {
            let key = ns
                .name()
                .map_or_else(|| "xmlns".to_owned(), |p| format!("xmlns:{p}"));
            element.set_attribute(key, ns.uri());
        }
    }
    element
}

fn fill(doc: &Document<'_>, node: Node<'_, '_>, element: &mut Element, file: &Rc<str>) {
    let mut pending = String::new();
    for child in node.children() {
        if child.is_text() {
            pending.push_str(child.text().unwrap_or_default());
        } else if child.is_element() {
            flush(&mut pending, element);
            let mut sub = element_shell(doc, child, file);
            fill(doc, child, &mut sub, file);
            element.add_child(sub);
        }
    }
    flush(&mut pending, element);
}

fn flush(pending: &mut String, element: &mut Element) {
    for line in pending.split('\n') {
        let line = line.trim_matches(|c: char| c.is_ascii() && is_c_space(c as u8));
        if !line.is_empty() {
            element.add_data(line);
        }
    }
    pending.clear();
}
