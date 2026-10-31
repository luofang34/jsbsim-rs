// SPDX-License-Identifier: LGPL-2.1-or-later
//! Filesystem-backed [`Resolver`](super::Resolver) for native front ends.

use std::path::PathBuf;

use crate::Error;

/// Resolves paths relative to a JSBSim root directory on disk.
///
/// Paths follow the platform's rules, as JSBSim's `SGPath` does: an
/// absolute path, such as an include's `file="/..."`, names that file
/// wherever it is, and a leading `..` reaches above the root. It is not a
/// sandbox; to load an untrusted tree, serve it through a
/// [`MemoryResolver`](super::MemoryResolver) or a [`Resolver`](super::Resolver)
/// of your own that confines its paths.
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
