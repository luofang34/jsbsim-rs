// SPDX-License-Identifier: LGPL-2.1-or-later
//! Filesystem-backed [`Resolver`](super::Resolver) for native front ends.

use std::path::PathBuf;

use crate::Error;

/// Resolves paths relative to a JSBSim root directory on disk.
#[derive(Debug, Clone)]
pub struct FileResolver {
    root: PathBuf,
}

impl FileResolver {
    /// A resolver for the JSBSim tree at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl super::Resolver for FileResolver {
    fn resolve(&self, path: &str) -> Result<Vec<u8>, Error> {
        std::fs::read(self.root.join(path)).map_err(|_| Error::ResourceNotFound {
            path: path.to_owned(),
        })
    }

    fn exists(&self, path: &str) -> bool {
        self.root.join(path).is_file()
    }
}
