// SPDX-License-Identifier: LGPL-2.1-or-later
//! Whole-corpus XML check: every XML file shipped with JSBSim must parse to
//! the same element tree in C++ and Rust, and every aircraft's `file`
//! includes must resolve or fail with a typed error.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use jsbsim::xml::{DocumentLoader, Element, IncludeKind};
use jsbsim::{Error, FileResolver};
use tracing::{error, info};

use crate::cases::report_transcript;
use crate::manifest::TranscriptSpec;
use crate::probe::{self, ProbeContext};
use crate::transcript::compare_transcript;
use crate::upstream::{self, Paths};
use crate::{OracleError, ToleranceTable};

/// Directories of the JSBSim tree that hold XML inputs.
pub const XML_DIRS: [&str; 5] = ["aircraft", "engine", "systems", "scripts", "tests"];

/// Every `.xml` file under [`XML_DIRS`], relative to `root`, sorted.
pub fn list_xml_files_blocking(root: &Path) -> Result<Vec<String>, OracleError> {
    let mut files = Vec::new();
    let mut pending: Vec<PathBuf> = XML_DIRS.iter().map(|d| root.join(d)).collect();
    while let Some(dir) = pending.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|e| OracleError::io("listing", &dir, e))?;
        for entry in entries {
            let path = entry
                .map_err(|e| OracleError::io("listing", &dir, e))?
                .path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "xml")
                && let Ok(rel) = path.strip_prefix(root)
            {
                files.push(rel.to_string_lossy().into_owned());
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Runs the corpus check; failure when the dumps differ or Rust panics.
pub fn check_xml_blocking(paths: &Paths) -> Result<ExitCode, OracleError> {
    let files = list_xml_files_blocking(&paths.jsbsim_root)?;
    let run_dir = paths.work_dir().join("run").join("corpus");
    std::fs::create_dir_all(&run_dir).map_err(|e| OracleError::io("creating", &run_dir, e))?;
    let input_path = run_dir.join("input.txt");
    let input: String = files.iter().map(|f| format!("file {f}\n")).collect();
    std::fs::write(&input_path, &input).map_err(|e| OracleError::io("writing", &input_path, e))?;
    let spec = TranscriptSpec {
        probe: "xml".to_owned(),
    };
    let reference = upstream::run_transcript_blocking(paths, &spec, &input_path, &run_dir)?;
    let context = ProbeContext {
        input_dir: run_dir.clone(),
        jsbsim_root: Some(paths.jsbsim_root.clone()),
    };
    let candidate = catch_unwind(AssertUnwindSafe(|| probe::run("xml", &input, &context)))
        .map_err(|_| OracleError::Provenance("the Rust XML parser panicked".to_owned()))??;
    let errors = reference.text.lines().filter(|l| *l == "ERROR").count();
    info!("corpus: {} files, {errors} rejected by C++", files.len());
    // The corpus has no tolerance rows: the dumps must be identical.
    let outcome = compare_transcript(
        "corpus",
        &reference.text,
        &candidate,
        &ToleranceTable::default(),
    )?;
    let mut code = if report_transcript("corpus", &outcome) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    };
    if !check_includes(&paths.jsbsim_root, &files) {
        code = ExitCode::FAILURE;
    }
    Ok(code)
}

/// Resolves the includes of every `aircraft/<name>/<name>.xml`. Returns
/// false only if the loader panicked; typed errors are reported.
fn check_includes(root: &Path, files: &[String]) -> bool {
    let resolver = FileResolver::new(root);
    let mut loaded = 0usize;
    let mut failed = 0usize;
    for file in files {
        let parts: Vec<&str> = file.split('/').collect();
        let [_, name, config] = parts.as_slice() else {
            continue;
        };
        if *config != format!("{name}.xml") {
            continue;
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            resolve_aircraft(&resolver, &format!("aircraft/{name}"), file)
        }));
        match outcome {
            Ok(Ok(includes)) => {
                loaded = loaded.wrapping_add(1);
                tracing::debug!("{file}: {includes} includes");
            }
            Ok(Err(e)) => {
                failed = failed.wrapping_add(1);
                error!("{file}: {e}");
            }
            Err(_) => {
                error!("{file}: include resolution panicked");
                return false;
            }
        }
    }
    info!("includes: {loaded} aircraft resolved, {failed} failed with typed errors");
    true
}

fn resolve_aircraft(resolver: &FileResolver, dir: &str, file: &str) -> Result<usize, Error> {
    let mut loader = DocumentLoader::new(resolver, dir);
    let mut config = loader.load_document(file)?;
    let mut includes = 0usize;
    for child in config.children_mut() {
        let kind = match child.name() {
            "system" => IncludeKind::System,
            _ => IncludeKind::Model,
        };
        if loader.open(child, kind)?.is_some() {
            includes = includes.wrapping_add(1);
        }
        if child.name() == "propulsion" {
            includes = includes.wrapping_add(resolve_engines(&mut loader, child)?);
        }
    }
    Ok(includes)
}

fn resolve_engines(
    loader: &mut DocumentLoader<'_>,
    propulsion: &mut Element,
) -> Result<usize, Error> {
    let mut includes = 0usize;
    for engine in propulsion
        .children_mut()
        .iter_mut()
        .filter(|e| e.name() == "engine")
    {
        if loader.open(engine, IncludeKind::Engine)?.is_some() {
            includes = includes.wrapping_add(1);
        }
        for thruster in engine
            .children_mut()
            .iter_mut()
            .filter(|t| t.name() == "thruster")
        {
            if loader.open(thruster, IncludeKind::Engine)?.is_some() {
                includes = includes.wrapping_add(1);
            }
        }
    }
    Ok(includes)
}
