// SPDX-License-Identifier: LGPL-2.1-or-later
//! Building and running the C++ JSBSim reference.
//!
//! The JSBSim checkout is only read: CMake configures it into a build
//! directory under `target/`, and the oracle driver is compiled against the
//! resulting static library. Scripts run with their working directory under
//! `target/` too, because aircraft may declare their own output files.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tracing::info;

use crate::manifest::{Provenance, ScriptSpec, TranscriptSpec};
use crate::{OracleError, Series};

#[cfg(test)]
mod tests;

/// Extra compiler flags for libJSBSim and the driver. Clang contracts
/// `a * b + c` into fused multiply-adds on targets that have them (arm64)
/// unless told not to; Rust never does, and neither does an x86-64 build
/// without `-march`, so references are built without contraction.
pub const CXX_FLAGS: &str = "-ffp-contract=off";

/// Driver sources, relative to the repository root.
pub const DRIVER_DIR: &str = "tools/oracle/cpp";

fn driver_sources_blocking(dir: &Path) -> Result<Vec<PathBuf>, OracleError> {
    let entries = std::fs::read_dir(dir).map_err(|e| OracleError::io("listing", dir, e))?;
    let mut sources = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| OracleError::io("listing", dir, e))?
            .path();
        if path.extension().is_some_and(|ext| ext == "cpp") {
            sources.push(path);
        }
    }
    sources.sort();
    Ok(sources)
}

/// Locations used by the oracle.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Root of this repository.
    pub repo_root: PathBuf,
    /// Read-only JSBSim checkout.
    pub jsbsim_root: PathBuf,
}

impl Paths {
    /// Scratch area for everything the oracle builds or writes.
    #[must_use]
    pub fn work_dir(&self) -> PathBuf {
        self.repo_root.join("target").join("oracle")
    }

    /// CMake build directory of the C++ JSBSim.
    #[must_use]
    pub fn build_dir(&self) -> PathBuf {
        self.work_dir().join("jsbsim-build")
    }

    /// Compiled driver executable.
    #[must_use]
    pub fn driver_exe(&self) -> PathBuf {
        self.work_dir().join("oracle_driver")
    }

    /// Directory holding the committed golden cases.
    #[must_use]
    pub fn goldens_dir(&self) -> PathBuf {
        self.repo_root.join("oracle").join("goldens")
    }

    /// Committed tolerance table.
    #[must_use]
    pub fn tolerances_file(&self) -> PathBuf {
        self.repo_root.join("oracle").join("tolerances.toml")
    }
}

pub(crate) fn run_blocking(command: &mut Command) -> Result<Output, OracleError> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command.output().map_err(|e| OracleError::Command {
        program: program.clone(),
        status: "could not start".to_owned(),
        detail: e.to_string(),
    })?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(OracleError::Command {
            program,
            status: output.status.to_string(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}

/// Commit of the JSBSim checkout. Fails if tracked files are modified, since
/// the commit would then not identify the code that ran.
pub fn upstream_commit_blocking(jsbsim_root: &Path) -> Result<String, OracleError> {
    let git = |args: &[&str]| {
        run_blocking(Command::new("git").arg("-C").arg(jsbsim_root).args(args))
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    let changes = git(&["status", "--porcelain", "--untracked-files=no"])?;
    if !changes.is_empty() {
        return Err(OracleError::Provenance(format!(
            "{} has tracked changes; references must come from a clean commit:\n{changes}",
            jsbsim_root.display()
        )));
    }
    git(&["rev-parse", "HEAD"])
}

/// Configures (once) and builds the JSBSim static library out of tree, then
/// compiles the driver against it.
pub fn build_blocking(paths: &Paths) -> Result<(), OracleError> {
    let build_dir = paths.build_dir();
    info!(
        "configuring {} into {}",
        paths.jsbsim_root.display(),
        build_dir.display()
    );
    run_blocking(
        Command::new("cmake")
            .arg("-S")
            .arg(&paths.jsbsim_root)
            .arg("-B")
            .arg(&build_dir)
            .args(["-G", "Ninja", "-DCMAKE_BUILD_TYPE=Release"])
            .args(["-DBUILD_DOCS=OFF", "-DBUILD_PYTHON_MODULE=OFF"])
            .arg(format!("-DCMAKE_CXX_FLAGS={CXX_FLAGS}"))
            .arg(format!("-DCMAKE_C_FLAGS={CXX_FLAGS}")),
    )?;
    info!("building libJSBSim");
    run_blocking(
        Command::new("ninja")
            .arg("-C")
            .arg(&build_dir)
            .arg("libJSBSim"),
    )?;
    info!("compiling {DRIVER_DIR}");
    let sources = driver_sources_blocking(&paths.repo_root.join(DRIVER_DIR))?;
    run_blocking(
        Command::new("c++")
            .args(["-std=c++17", "-O2", CXX_FLAGS, "-I"])
            .arg(paths.jsbsim_root.join("src"))
            .arg("-I")
            .arg(build_dir.join("src"))
            .args(&sources)
            .arg(build_dir.join("src").join("libJSBSim.a"))
            .arg("-o")
            .arg(paths.driver_exe()),
    )?;
    Ok(())
}

/// Output of one driver run.
#[derive(Debug, Clone)]
pub struct Run {
    /// Sampled values.
    pub series: Series,
    /// Where the values came from.
    pub provenance: Provenance,
}

/// Runs `spec` through the compiled driver with its working directory in
/// `run_dir`, and checks that every requested property was logged.
pub fn run_script_blocking(
    paths: &Paths,
    spec: &ScriptSpec,
    run_dir: &Path,
) -> Result<Run, OracleError> {
    run_script_in_blocking(paths, &paths.jsbsim_root, spec, run_dir)
}

/// As [`run_script_blocking`], with the script and aircraft found under
/// `root` instead of the JSBSim checkout.
pub fn run_script_in_blocking(
    paths: &Paths,
    root: &Path,
    spec: &ScriptSpec,
    run_dir: &Path,
) -> Result<Run, OracleError> {
    let upstream_commit = upstream_commit_blocking(&paths.jsbsim_root)?;
    std::fs::create_dir_all(run_dir).map_err(|e| OracleError::io("creating", run_dir, e))?;
    let csv = run_dir.join("driver.csv");
    let end = spec
        .end_time
        .map_or_else(|| "-".to_owned(), |t| format!("{t:?}"));
    let output = run_blocking(
        Command::new(paths.driver_exe())
            .current_dir(run_dir)
            .env("JSBSIM_DEBUG", "0")
            .env(
                "ORACLE_AIRCRAFT_PATH",
                spec.aircraft_path.as_deref().unwrap_or("aircraft"),
            )
            .arg("script")
            .arg(root)
            .arg(root.join(&spec.script))
            .arg(format!("{:?}", spec.rate_hz))
            .arg(end)
            .arg(&csv)
            .args(&spec.properties),
    )?;
    let (dt, every_n_frames) = parse_timing(&String::from_utf8_lossy(&output.stdout))?;
    let series = Series::read_blocking(&csv)?;
    let expected: Vec<&str> = std::iter::once(crate::series::TIME_COLUMN)
        .chain(spec.properties.iter().map(String::as_str))
        .collect();
    if series.columns != expected {
        return Err(OracleError::ColumnMismatch {
            reference: expected.into_iter().map(str::to_owned).collect(),
            candidate: series.columns,
        });
    }
    let rows = series.rows.len();
    Ok(Run {
        series,
        provenance: Provenance {
            upstream_commit,
            driver: DRIVER_DIR.to_owned(),
            cxx_flags: CXX_FLAGS.to_owned(),
            dt: Some(dt),
            every_n_frames: Some(every_n_frames),
            rows,
        },
    })
}

/// Output of one probe run.
#[derive(Debug, Clone)]
pub struct Transcript {
    /// The probe's output.
    pub text: String,
    /// Where the output came from.
    pub provenance: Provenance,
}

/// Runs the driver's `spec.probe` mode on `input`, in `run_dir`.
pub fn run_transcript_blocking(
    paths: &Paths,
    spec: &TranscriptSpec,
    input: &Path,
    run_dir: &Path,
) -> Result<Transcript, OracleError> {
    let upstream_commit = upstream_commit_blocking(&paths.jsbsim_root)?;
    std::fs::create_dir_all(run_dir).map_err(|e| OracleError::io("creating", run_dir, e))?;
    let out = run_dir.join("driver.txt");
    run_blocking(
        Command::new(paths.driver_exe())
            .current_dir(run_dir)
            .env("JSBSIM_DEBUG", "0")
            .env("JSBSIM_ROOT", &paths.jsbsim_root)
            .arg(&spec.probe)
            .arg(input)
            .arg(&out),
    )?;
    let text = std::fs::read_to_string(&out).map_err(|e| OracleError::io("reading", &out, e))?;
    Ok(Transcript {
        provenance: Provenance {
            upstream_commit,
            driver: DRIVER_DIR.to_owned(),
            cxx_flags: CXX_FLAGS.to_owned(),
            dt: None,
            every_n_frames: None,
            rows: text.lines().count(),
        },
        text,
    })
}

/// Extracts `dt=<f64> every_n_frames=<u64>` from the driver's stdout, which
/// also carries JSBSim's own console messages.
pub fn parse_timing(stdout: &str) -> Result<(f64, u64), OracleError> {
    let missing = || OracleError::Provenance(format!("driver printed no timing line:\n{stdout}"));
    let line = stdout
        .lines()
        .find(|line| line.starts_with("dt="))
        .ok_or_else(missing)?;
    let mut fields = line.split_whitespace();
    let dt = fields
        .next()
        .and_then(|f| f.strip_prefix("dt="))
        .and_then(|v| v.parse().ok());
    let every = fields
        .next()
        .and_then(|f| f.strip_prefix("every_n_frames="))
        .and_then(|v| v.parse().ok());
    dt.zip(every).ok_or_else(missing)
}
