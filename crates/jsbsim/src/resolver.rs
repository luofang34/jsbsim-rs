// SPDX-License-Identifier: LGPL-2.1-or-later
//! Access to model and data files without touching the filesystem.
//!
//! The core asks a [`Resolver`] for bytes by path; native front ends can read
//! files, browsers can serve embedded or fetched bytes.

use std::collections::BTreeMap;

use crate::Error;

#[cfg(test)]
mod tests;

/// Source of the bytes behind a JSBSim resource path such as
/// `aircraft/ball/ball.xml`.
pub trait Resolver {
    /// Returns the contents of `path`, or [`Error::ResourceNotFound`].
    fn resolve(&self, path: &str) -> Result<Vec<u8>, Error>;
}

/// Resolver backed by an in-memory map from path to bytes.
#[derive(Debug, Default, Clone)]
pub struct MemoryResolver {
    files: BTreeMap<String, Vec<u8>>,
}

impl MemoryResolver {
    /// Creates an empty resolver.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces the resource at `path`.
    pub fn insert(&mut self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) {
        self.files.insert(path.into(), bytes.into());
    }
}

impl Resolver for MemoryResolver {
    fn resolve(&self, path: &str) -> Result<Vec<u8>, Error> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| Error::ResourceNotFound {
                path: path.to_owned(),
            })
    }
}
