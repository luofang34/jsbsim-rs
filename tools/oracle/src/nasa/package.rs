// SPDX-License-Identifier: LGPL-2.1-or-later
//! The F-16's DAVE-ML sources, unpacked from the pinned Atmospheric_models
//! archive and checked against the digests oracle/nasa/aircraft/f16/f16.xml
//! is translated from.

use std::path::PathBuf;
use std::process::Command;

use jsbsim::oracle_support::xml::{Element, parse_document};

use super::{NasaManifest, cache_dir, sha256_hex, verify};
use crate::OracleError;
use crate::upstream::{Paths, run_blocking};

/// The archive holding F16_package.zip.
const ARCHIVE: &str = "Atmospheric_models";
/// SHA-256 of its member F16_package.zip.
const PACKAGE_SHA256: &str = "20c60f615ae8e87d81c9d98b54fff45a2832840201499cbcfe3f45a60ef3e5b2";
/// The models' directory inside F16_package.zip.
const SOURCE_DIR: &str = "F16_package/F16_S119_source";
/// SHA-256 of each model f16.xml is translated from.
const MODELS: [(&str, &str); 5] = [
    (
        "F16_aero.dml",
        "82796577b989b5f3becb7d13430ff6c619b3ecd2e642a6427b743658fb177e51",
    ),
    (
        "F16_inertia.dml",
        "e11d4eba5b158d24db7034bd85682ad0b896dd2f3172f007ca3d6d6d4e693c4a",
    ),
    (
        "F16_prop.dml",
        "e5fd7772e5c05cdac666c94febbe4cb50139418b26045affd92dbd198f2215c6",
    ),
    (
        "F16_control.dml",
        "bca7789dde61b741de69f697ce8799ee666214d0e1fa203973523774fc3a969b",
    ),
    (
        "F16_gnc.dml",
        "fc1dbff00cc14b0228973c43cfc17797fd48d2e36950ec039ff4e77d8577827c",
    ),
];

fn fail(message: String) -> OracleError {
    OracleError::NasaData {
        archive: ARCHIVE.to_owned(),
        message,
    }
}

fn read_blocking(path: &std::path::Path) -> Result<Vec<u8>, OracleError> {
    std::fs::read(path).map_err(|e| OracleError::io("reading", path, e))
}

/// Unpacks the F-16 models from the cached archive (after `oracle nasa
/// fetch`) and returns their directory.
pub(super) fn unpack_blocking(paths: &Paths) -> Result<PathBuf, OracleError> {
    let manifest = NasaManifest::read_blocking(&paths.repo_root)?;
    let dir = cache_dir(paths);
    let zip = dir.join(format!("{ARCHIVE}.zip"));
    let bytes =
        read_blocking(&zip).map_err(|e| fail(format!("{e}; run `oracle nasa fetch` first")))?;
    verify(manifest.archive(ARCHIVE)?, &bytes)?;
    let package = dir.join(ARCHIVE).join("F16_package.zip");
    let digest = sha256_hex(&read_blocking(&package)?);
    if digest != PACKAGE_SHA256 {
        return Err(fail(format!(
            "F16_package.zip has sha256 {digest}, f16.xml was translated from {PACKAGE_SHA256}"
        )));
    }
    let out = dir.join("F16_package");
    run_blocking(
        Command::new("unzip")
            .args(["-q", "-o"])
            .arg(&package)
            .arg(format!("{SOURCE_DIR}/*.dml"))
            .arg("-d")
            .arg(&out),
    )?;
    let models = out.join(SOURCE_DIR);
    for (name, pinned) in MODELS {
        let digest = sha256_hex(&read_blocking(&models.join(name))?);
        if digest != pinned {
            return Err(fail(format!(
                "{name} has sha256 {digest}, f16.xml was translated from {pinned}"
            )));
        }
    }
    Ok(models)
}

/// Parses model `name` of the unpacked directory `models`.
pub(super) fn model_blocking(models: &std::path::Path, name: &str) -> Result<Element, OracleError> {
    let path = models.join(name);
    parse_document(&read_blocking(&path)?, name).map_err(|e| OracleError::DaveMl {
        source_name: name.to_owned(),
        message: e.to_string(),
    })
}
