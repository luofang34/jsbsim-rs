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
#[non_exhaustive]
pub struct PathError {
    /// SimGear's message for the problem.
    pub message: String,
}

/// One step of a parsed path, borrowing its name from the path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Component<'a> {
    /// Leading `/`: start from the root.
    Root,
    /// `.`
    Current,
    /// `..`
    Parent,
    /// A child name with its index (0 when no `[n]` is given).
    Child(&'a str, i32),
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
/// including its quirk that `a[1]b` reads as `a[1]/b`, with `index` in
/// place of the last component's index when that is a child. The whole
/// path is checked before the first component is returned, as SimGear
/// parses it before walking, and nothing is allocated.
pub(crate) fn parse_path(path: &str, index: Option<i32>) -> Result<Components<'_>, PathError> {
    let components = Components::new(path, index);
    let mut pos = components.pos;
    while pos < path.len() {
        pos = skip_slashes(path.as_bytes(), parse_component(path, pos)?.1);
    }
    Ok(components)
}

/// The components of a path [`parse_path`] has checked.
pub(crate) struct Components<'a> {
    path: &'a str,
    pos: usize,
    root: bool,
    index: Option<i32>,
}

impl<'a> Components<'a> {
    fn new(path: &'a str, index: Option<i32>) -> Self {
        let root = path.starts_with('/');
        let pos = if root {
            skip_slashes(path.as_bytes(), 1)
        } else {
            0
        };
        Self {
            path,
            pos,
            root,
            index,
        }
    }
}

impl<'a> Iterator for Components<'a> {
    type Item = Component<'a>;

    fn next(&mut self) -> Option<Component<'a>> {
        if self.root {
            self.root = false;
            return Some(Component::Root);
        }
        if self.pos >= self.path.len() {
            return None;
        }
        // parse_path checked every component, so none fails here.
        let (component, next) = parse_component(self.path, self.pos).ok()?;
        self.pos = skip_slashes(self.path.as_bytes(), next);
        match (component, self.index) {
            (Component::Child(name, _), Some(index)) if self.pos >= self.path.len() => {
                Some(Component::Child(name, index))
            }
            _ => Some(component),
        }
    }
}

fn skip_slashes(bytes: &[u8], mut pos: usize) -> usize {
    while bytes.get(pos) == Some(&b'/') {
        pos += 1;
    }
    pos
}

fn parse_component(path: &str, start: usize) -> Result<(Component<'_>, usize), PathError> {
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
    let name = path.get(start..i).unwrap_or_default();
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
