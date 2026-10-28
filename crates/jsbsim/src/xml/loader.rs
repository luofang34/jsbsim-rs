// SPDX-License-Identifier: LGPL-2.1-or-later
// FGModelLoader: Copyright (C) 2013 Bertrand Coconnier.
// FGXMLFileRead: Copyright (C) 2007 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/input_output/FGModelLoader.cpp, FGXMLFileRead.cpp
// and the FindFullPathName overrides of models/FGModel.cpp, FGFCS.cpp and
// FGPropulsion.cpp.
//
//! Loading documents through a [`Resolver`] and resolving `file`
//! attributes the way JSBSim's models do.

use std::collections::BTreeMap;

use super::{Element, XmlError, parse_document};
use crate::{Error, Resolver};

/// Which model resolves a `file` attribute; each searches its own
/// directories, in JSBSim's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncludeKind {
    /// Any model: the aircraft directory only.
    Model,
    /// `<system>`: the aircraft directory, its `Systems` and `systems`
    /// subdirectories, then the systems directory.
    System,
    /// `<engine>` and `<thruster>`: the aircraft directory, its `Engines`,
    /// `engines`, `Engine` and `engine` subdirectories, then the engine
    /// directory.
    Engine,
}

/// Loads XML documents and resolves includes relative to one aircraft.
pub struct DocumentLoader<'r> {
    resolver: &'r dyn Resolver,
    aircraft_dir: String,
    systems_dir: String,
    engine_dir: String,
    cache: BTreeMap<String, Element>,
}

impl<'r> DocumentLoader<'r> {
    /// A loader for the aircraft in `aircraft_dir` (e.g. `aircraft/c172x`),
    /// with JSBSim's default `systems` and `engine` directories.
    pub fn new(resolver: &'r dyn Resolver, aircraft_dir: impl Into<String>) -> Self {
        Self {
            resolver,
            aircraft_dir: aircraft_dir.into(),
            systems_dir: "systems".to_owned(),
            engine_dir: "engine".to_owned(),
            cache: BTreeMap::new(),
        }
    }

    /// The loader with `dir` as the systems directory (`SetSystemsPath`).
    #[must_use]
    pub fn with_systems_dir(mut self, dir: impl Into<String>) -> Self {
        self.systems_dir = dir.into();
        self
    }

    /// `FGXMLFileRead::LoadXMLDocument`: reads and parses `path`, adding
    /// `.xml` when it has no extension.
    pub fn load_document(&self, path: &str) -> Result<Element, Error> {
        let path = if extension(path).is_empty() {
            format!("{path}.xml")
        } else {
            path.to_owned()
        };
        let path = normalize(&path);
        let bytes = self.resolver.resolve(&path)?;
        Ok(parse_document(&bytes, &path)?)
    }

    /// `FGModelLoader::Open`: when `element` has a `file` attribute, loads
    /// that document; if its root has another name, it is also appended to
    /// `element`'s children. `None` when there is no `file` attribute.
    pub fn open(
        &mut self,
        element: &mut Element,
        kind: IncludeKind,
    ) -> Result<Option<Element>, Error> {
        let file = element.attribute("file").to_owned();
        if file.is_empty() {
            return Ok(None);
        }
        let path = if file.starts_with('/') {
            Some(file.clone())
        } else {
            self.find_full_path(&file, kind)
        };
        let not_found = || XmlError::IncludeNotFound {
            file: file.clone(),
            at: element.location(),
        };
        let path = path.ok_or_else(not_found)?;
        let document = match self.cache.get(&path) {
            Some(cached) => cached.clone(),
            None => {
                let document = self.load_document(&path).map_err(|e| match e {
                    Error::ResourceNotFound { .. } => not_found().into(),
                    other => other,
                })?;
                self.cache.insert(path, document.clone());
                document
            }
        };
        if document.name() != element.name() {
            element.add_child(document.clone());
        }
        Ok(Some(document))
    }

    fn find_full_path(&self, file: &str, kind: IncludeKind) -> Option<String> {
        let aircraft = self.aircraft_dir.as_str();
        let mut dirs = vec![aircraft.to_owned()];
        let (subdirs, fallback): (&[&str], &str) = match kind {
            IncludeKind::Model => (&[], ""),
            IncludeKind::System => (&["Systems", "systems"], &self.systems_dir),
            IncludeKind::Engine => (
                &["Engines", "engines", "Engine", "engine"],
                &self.engine_dir,
            ),
        };
        dirs.extend(subdirs.iter().map(|sub| format!("{aircraft}/{sub}")));
        if kind != IncludeKind::Model {
            dirs.push(fallback.to_owned());
        }
        dirs.iter().find_map(|dir| self.check_path_name(dir, file))
    }

    /// `CheckPathName`: `dir/file`, with `.xml` appended unless that is
    /// already the extension, if it exists.
    fn check_path_name(&self, dir: &str, file: &str) -> Option<String> {
        let mut full = format!("{dir}/{file}");
        if extension(&full) != "xml" {
            full.push_str(".xml");
        }
        let full = normalize(&full);
        self.resolver.exists(&full).then_some(full)
    }
}

/// `SGPath::extension`: the text after the last `.` of the file name.
fn extension(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rfind('.').map_or("", |at| &name[at + 1..])
}

/// Removes `.` and `name/..` segments so in-memory resolvers can match
/// paths that the operating system would resolve.
fn normalize(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|p| *p != "..") => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    let joined = parts.join("/");
    if absolute {
        format!("/{joined}")
    } else {
        joined
    }
}
