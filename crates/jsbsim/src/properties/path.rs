// SPDX-License-Identifier: LGPL-2.1-or-later
// props.cxx: Started Fall 2000 by David Megginson, david@megginson.com.
// This code is released into the Public Domain.
// Derived from JSBSim src/simgear/props/props.cxx (standalone path parser).
//
//! Property path parsing: `Path: "/"? Component ("/"+ Component)*`,
//! `Component: Name Index? | "." | ".."`, `Name: [_a-zA-Z][-._a-zA-Z0-9]*`,
//! `Index: "[" [0-9]* "]"`.

use thiserror::Error;

/// A property path that SimGear would reject.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct PathError {
    /// SimGear's message for the problem.
    pub message: String,
}

/// One step of a parsed path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Component {
    /// Leading `/`: start from the root.
    Root,
    /// `.`
    Current,
    /// `..`
    Parent,
    /// A child name with its index (0 when no `[n]` is given).
    Child(String, i32),
}

fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')
}

fn error(message: String) -> PathError {
    PathError { message }
}

/// Splits `path` into components the way SimGear's `parse_path` does,
/// including its quirk that `a[1]b` reads as `a[1]/b`.
pub(crate) fn parse_path(path: &str) -> Result<Vec<Component>, PathError> {
    let bytes = path.as_bytes();
    let mut components = Vec::new();
    let mut pos = 0;
    if bytes.first() == Some(&b'/') {
        components.push(Component::Root);
        pos = skip_slashes(bytes, 1);
    }
    while pos < bytes.len() {
        let (component, next) = parse_component(path, pos)?;
        components.push(component);
        pos = skip_slashes(bytes, next);
    }
    Ok(components)
}

fn skip_slashes(bytes: &[u8], mut pos: usize) -> usize {
    while bytes.get(pos) == Some(&b'/') {
        pos += 1;
    }
    pos
}

fn parse_component(path: &str, start: usize) -> Result<(Component, usize), PathError> {
    let bytes = path.as_bytes();
    let mut i = start;
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        let component = if bytes.get(i) == Some(&b'.') {
            i += 1;
            Component::Parent
        } else {
            Component::Current
        };
        if bytes.get(i).is_some_and(|&b| b != b'/') {
            let name = if component == Component::Parent {
                ".."
            } else {
                "."
            };
            return Err(error(format!("Illegal character after {name} in '{path}'")));
        }
        return Ok((component, i));
    }
    if !bytes.get(i).copied().is_some_and(is_name_start) {
        return Err(error(format!(
            "name must begin with alpha or '_' in '{path}'"
        )));
    }
    i += 1;
    while let Some(&b) = bytes.get(i) {
        if is_name_char(b) {
            i += 1;
        } else if b == b'[' || b == b'/' {
            break;
        } else {
            return Err(error(format!(
                "name may contain only ._- and alphanumeric characters in '{path}'"
            )));
        }
    }
    let name = path.get(start..i).unwrap_or_default().to_owned();
    let (index, next) = parse_index(path, i)?;
    Ok((Component::Child(name, index), next))
}

fn parse_index(path: &str, start: usize) -> Result<(i32, usize), PathError> {
    let bytes = path.as_bytes();
    if bytes.get(start) != Some(&b'[') {
        return Ok((0, start));
    }
    let mut index: i32 = 0;
    let mut i = start + 1;
    while let Some(&b) = bytes.get(i) {
        if b.is_ascii_digit() {
            index = index.wrapping_mul(10).wrapping_add(i32::from(b - b'0'));
        } else if b == b']' {
            return Ok((index, i + 1));
        } else {
            break;
        }
        i += 1;
    }
    Err(error(format!(
        "unterminated index (looking for ']') in '{path}'"
    )))
}
