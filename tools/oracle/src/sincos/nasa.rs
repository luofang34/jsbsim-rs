// SPDX-License-Identifier: LGPL-2.1-or-later
//! The [`super::NASA_CASES`] half of `check-sincos`: a NASA check-case
//! sampled as `oracle nasa compare` samples it, with the separate-sincos
//! C++ build as the reference.

use super::{reference_dir, report_series};
use crate::OracleError;
use crate::manifest::REFERENCE_FILE;
use crate::nasa::{self, NasaManifest};
use crate::upstream::{self, Paths};

/// Regenerates NASA case `name` with the separate-sincos C++ build, runs
/// the port on it and logs the outcome; returns whether they are
/// bit-identical.
pub(super) fn check_case_blocking(
    paths: &Paths,
    manifest: &NasaManifest,
    name: &str,
) -> Result<bool, OracleError> {
    let Some(case) = manifest.case.iter().find(|c| c.name == name) else {
        return Err(OracleError::Sincos(format!(
            "{name}: not a case of oracle/nasa/manifest.toml"
        )));
    };
    let spec = nasa::spec(case);
    let (cpp_root, rust_root) = nasa::roots(paths, case);
    let label = format!("nasa-{name}");
    let run_dir = paths.work_dir().join("run").join(&label);
    let reference = upstream::run_script_in_blocking(paths, &cpp_root, &spec, &run_dir)?.series;
    let dir = reference_dir(paths, &label);
    std::fs::create_dir_all(&dir).map_err(|e| OracleError::io("creating", &dir, e))?;
    reference.write_blocking(&dir.join(REFERENCE_FILE))?;
    let rust = crate::port_script::run_script(&rust_root, &spec);
    Ok(report_series(name, &reference, rust))
}
