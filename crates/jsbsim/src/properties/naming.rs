// SPDX-License-Identifier: LGPL-2.1-or-later
// props.cxx: Started Fall 2000 by David Megginson, david@megginson.com.
// This code is released into the Public Domain.
// FGPropertyManager: Copyright (C) 2002 Tony Peden.
// Derived from JSBSim src/simgear/props/props.cxx (getPath, getDisplayName)
// and src/input_output/FGPropertyManager.cpp (name helpers).
//
//! Node names and paths as JSBSim prints them.

use super::{NodeId, PathError, PropertyTree};

impl PropertyTree {
    /// `getDisplayName`: the name, followed by `[index]` unless `simplify`
    /// is set and the index is 0.
    #[must_use]
    pub fn display_name(&self, id: NodeId, simplify: bool) -> String {
        let index = self.index(id);
        if index != 0 || !simplify {
            format!("{}[{index}]", self.name(id))
        } else {
            self.name(id).to_owned()
        }
    }

    /// `getPath`: `/`-separated display names from the root; empty for the
    /// root itself.
    #[must_use]
    pub fn path(&self, id: NodeId, simplify: bool) -> String {
        let mut names = Vec::new();
        let mut current = id;
        while let Some(parent) = self.parent(current) {
            names.push(self.display_name(current, simplify));
            current = parent;
        }
        names.iter().rev().fold(String::new(), |mut path, name| {
            path.push('/');
            path.push_str(name);
            path
        })
    }

    /// `GetFullyQualifiedName`: like [`path`](Self::path) with simplified
    /// names, and `/` for the root.
    #[must_use]
    pub fn fully_qualified_name(&self, id: NodeId) -> String {
        match self.parent(id) {
            None => "/".to_owned(),
            Some(_) => self.path(id, true),
        }
    }

    /// `GetRelativeName`: the fully qualified name with the prefix `path`
    /// removed when present.
    #[must_use]
    pub fn relative_name(&self, id: NodeId, path: &str) -> String {
        let name = self.fully_qualified_name(id);
        match name.strip_prefix(path) {
            Some(rest) if !path.is_empty() => rest.to_owned(),
            _ => name,
        }
    }

    /// Aliases `id` to the node at `path` relative to `id`, creating it
    /// (`alias(const char*)`).
    pub fn alias_path(&mut self, id: NodeId, path: &str) -> Result<bool, PathError> {
        let target = self.get_or_create(id, path)?;
        Ok(self.alias(id, target))
    }
}

/// `FGPropertyManager::mkPropertyName`: spaces become `-`, and upper case
/// becomes lower case when `lowercase` is set.
#[must_use]
pub fn mk_property_name(name: &str, lowercase: bool) -> String {
    let bytes: Vec<u8> = name
        .bytes()
        .map(|b| {
            if lowercase && b.is_ascii_uppercase() {
                b.to_ascii_lowercase()
            } else if crate::c_numeric::is_c_space(b) {
                b'-'
            } else {
                b
            }
        })
        .collect();
    // Only ASCII bytes were replaced by ASCII bytes, so UTF-8 is intact.
    String::from_utf8_lossy(&bytes).into_owned()
}
