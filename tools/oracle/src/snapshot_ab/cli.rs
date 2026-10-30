// SPDX-License-Identifier: LGPL-2.1-or-later
//! `oracle snapshot`: snapshots across targets (scripts/wasm-determinism.sh).
//!
//! `dump` runs each case to its checkpoint, writes the snapshot's bytes
//! (`<case>.snap`), the fingerprint (`<case>.fingerprint`), the digest of
//! each frame of the continuation (`<case>.trace`, see
//! [`Frame::digest`](super::Frame::digest)) and the outcome of an A/B check
//! on this target (`<case>.ab`). `continue` loads each case afresh,
//! restores the snapshot another run dumped, and writes its fingerprint
//! and continuation. `compare` requires two directories to agree on every
//! file both hold, and every A/B check to have passed. `cost` times
//! `snapshot_into` and `restore`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::{Arg, ArgMatches, Command, value_parser};
use jsbsim::Fdm;
use jsbsim::snapshot::Snapshot;
use tracing::{error, info};

use super::{ab, record};
use crate::OracleError;
use crate::steady_state::{SCRIPTS, ScriptRun, TRIMMED};

/// Frames run from the initial conditions to the snapshot.
pub const CHECKPOINT: u32 = 240;

/// Frames of each continuation.
pub const CONTINUATION: u32 = 120;

/// The scripts of `oracle::steady_state` the cross-target check flies:
/// the c172x with its sensor noise, cruising and trimming, turbulence,
/// a cosine gust, a ground callback, and every engine and buoyancy
/// family.
pub const CASES: &[&str] = &[
    "scripts/ball.xml",
    "scripts/c1722.xml",
    "scripts/c172_cruise_8K.xml",
    "scripts/737_cruise.xml",
    "scripts/p51d_tail_wind.xml",
    "scripts/Test_F450_Launch.xml",
    "scripts/gear_rover_deck_rotating.xml",
    "scripts/piston_start_c310.xml",
    "scripts/f16_test.xml",
    "scripts/L4102.xml",
    "scripts/J2461.xml",
    "scripts/sim_primer.xml",
    "scripts/Submarine_Scout_1.xml",
];

/// The `snapshot` subcommand.
#[must_use]
pub fn command() -> Command {
    let dir = |name: &'static str, help: &'static str| {
        Arg::new(name)
            .required(true)
            .value_parser(value_parser!(PathBuf))
            .help(help)
    };
    let out = Arg::new("out")
        .long("out")
        .required(true)
        .value_parser(value_parser!(PathBuf))
        .help("Directory to write to");
    Command::new("snapshot")
        .about("Snapshot cases and continue them, on this target or from another's snapshots")
        .subcommand_required(true)
        .subcommand(
            Command::new("dump")
                .about("Snapshot each case, continue it, and check A against B")
                .arg(out.clone()),
        )
        .subcommand(
            Command::new("continue")
                .about("Restore each case from another run's snapshots and continue it")
                .arg(
                    Arg::new("from")
                        .long("from")
                        .required(true)
                        .value_parser(value_parser!(PathBuf))
                        .help("Directory a dump wrote"),
                )
                .arg(out),
        )
        .subcommand(
            Command::new("compare")
                .about("Require two directories to agree on every file both hold")
                .arg(dir("a", "One directory"))
                .arg(dir("b", "The other")),
        )
        .subcommand(Command::new("cost").about("Time snapshot_into and restore per frame"))
}

/// Runs the `snapshot` action selected in `matches`.
pub fn run_blocking(repo_root: &Path, matches: &ArgMatches) -> Result<ExitCode, OracleError> {
    let Some((name, sub)) = matches.subcommand() else {
        return Ok(ExitCode::FAILURE);
    };
    let dir = |name: &str| sub.get_one::<PathBuf>(name).cloned().unwrap_or_default();
    match name {
        "dump" => dump_blocking(repo_root, &dir("out")).map(|()| ExitCode::SUCCESS),
        "continue" => {
            continue_blocking(repo_root, &dir("from"), &dir("out")).map(|()| ExitCode::SUCCESS)
        }
        "compare" => compare_blocking(&dir("a"), &dir("b")),
        "cost" => cost_blocking(repo_root).map(|()| ExitCode::SUCCESS),
        _ => Ok(ExitCode::FAILURE),
    }
}

fn spec(case: &str) -> Result<&'static ScriptRun, OracleError> {
    SCRIPTS
        .iter()
        .chain(TRIMMED)
        .find(|s| s.script == case)
        .ok_or_else(|| failure(case, "not a run of oracle::steady_state".to_owned()))
}

fn failure(case: &str, message: String) -> OracleError {
    OracleError::Snapshot {
        case: case.to_owned(),
        message,
    }
}

fn port(case: &str) -> impl Fn(jsbsim::Error) -> OracleError + '_ {
    move |source| OracleError::SnapshotPort {
        case: case.to_owned(),
        source,
    }
}

/// The case loaded and initialised, not yet run.
fn load(repo_root: &Path, case: &str) -> Result<Fdm, OracleError> {
    let mut fdm = spec(case)?
        .load(&repo_root.join("oracle"))
        .map_err(|stop| failure(case, stop.to_string()))?;
    fdm.run_ic().map_err(port(case))?;
    Ok(fdm)
}

/// The case run to its checkpoint.
fn at_checkpoint(repo_root: &Path, case: &str) -> Result<Fdm, OracleError> {
    let mut fdm = load(repo_root, case)?;
    for _ in 0..CHECKPOINT {
        if !fdm.run().map_err(port(case))? {
            return Err(failure(
                case,
                "the run ended before the checkpoint".to_owned(),
            ));
        }
    }
    Ok(fdm)
}

/// The file `<case stem>.<ext>` in `dir`.
fn file(dir: &Path, case: &str, ext: &str) -> PathBuf {
    let stem = Path::new(case)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned());
    dir.join(format!("{}.{ext}", stem.unwrap_or_default()))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), OracleError> {
    std::fs::write(path, bytes).map_err(|e| OracleError::io("writing", path, e))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, b| {
        write!(out, "{b:02x}").ok();
        out
    })
}

/// The fingerprint and the continuation's frame digests, one per line.
fn write_continuation(dir: &Path, case: &str, fdm: &mut Fdm) -> Result<(), OracleError> {
    let fingerprint = fdm.fingerprint().map_err(port(case))?;
    write(
        &file(dir, case, "fingerprint"),
        hex(&fingerprint.to_bytes()).as_bytes(),
    )?;
    let frames = record(fdm, CONTINUATION, &mut |_| Ok(())).map_err(port(case))?;
    let trace: String = frames.iter().map(|f| hex(&f.digest()) + "\n").collect();
    write(&file(dir, case, "trace"), trace.as_bytes())
}

fn dump_blocking(repo_root: &Path, out: &Path) -> Result<(), OracleError> {
    std::fs::create_dir_all(out).map_err(|e| OracleError::io("creating", out, e))?;
    for case in CASES {
        let mut fdm = at_checkpoint(repo_root, case)?;
        let mut snapshot = Snapshot::new();
        fdm.snapshot_into(&mut snapshot).map_err(port(case))?;
        write(&file(out, case, "snap"), snapshot.as_bytes())?;
        write_continuation(out, case, &mut fdm)?;
        let mut fdm = at_checkpoint(repo_root, case)?;
        let outcome = ab(&mut fdm, CONTINUATION).map_err(port(case))?;
        let verdict = match (&outcome.mismatch, outcome.resnapshot_identical) {
            (None, true) => format!("ok {} frames\n", outcome.frames),
            (mismatch, identical) => {
                format!("FAILED {mismatch:?}, resnapshot identical {identical}\n")
            }
        };
        write(&file(out, case, "ab"), verdict.as_bytes())?;
        info!(
            "{case}: {} snapshot bytes, A/B {}",
            snapshot.as_bytes().len(),
            verdict.trim()
        );
    }
    Ok(())
}

fn continue_blocking(repo_root: &Path, from: &Path, out: &Path) -> Result<(), OracleError> {
    std::fs::create_dir_all(out).map_err(|e| OracleError::io("creating", out, e))?;
    for case in CASES {
        let path = file(from, case, "snap");
        let bytes = std::fs::read(&path).map_err(|e| OracleError::io("reading", &path, e))?;
        let snapshot =
            Snapshot::from_bytes(&bytes).map_err(|e| port(case)(jsbsim::Error::Snapshot(e)))?;
        let mut fdm = load(repo_root, case)?;
        fdm.restore(&snapshot).map_err(port(case))?;
        write_continuation(out, case, &mut fdm)?;
        info!("{case}: restored and continued");
    }
    Ok(())
}

fn compare_blocking(a: &Path, b: &Path) -> Result<ExitCode, OracleError> {
    let mut failures = 0;
    for case in CASES {
        for ext in ["snap", "fingerprint", "trace", "ab"] {
            let (pa, pb) = (file(a, case, ext), file(b, case, ext));
            let (Ok(da), Ok(db)) = (std::fs::read(&pa), std::fs::read(&pb)) else {
                continue;
            };
            if ext == "ab" && !(da.starts_with(b"ok") && db.starts_with(b"ok")) {
                error!(
                    "{case}: an A/B check failed: {}",
                    String::from_utf8_lossy(&db)
                );
                failures += 1;
            } else if ext != "ab" && da != db {
                error!("{case}: {} and {} differ", pa.display(), pb.display());
                failures += 1;
            }
        }
    }
    if failures == 0 {
        info!("snapshot: {} cases agree", CASES.len());
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::FAILURE)
    }
}

/// Mean nanoseconds of `f` over `n` calls.
fn mean_ns(n: u32, mut f: impl FnMut() -> Result<(), jsbsim::Error>) -> Result<f64, jsbsim::Error> {
    let start = Instant::now();
    for _ in 0..n {
        f()?;
    }
    Ok(start.elapsed().as_nanos() as f64 / f64::from(n))
}

/// Times a frame, `snapshot_into` and `restore` on the c172x cruising
/// (c172_cruise_8K, trimmed) and the ah1s (sim_primer, its rotors
/// governed), on a reused snapshot.
fn cost_blocking(repo_root: &Path) -> Result<(), OracleError> {
    const N: u32 = 2000;
    for case in ["scripts/c172_cruise_8K.xml", "scripts/sim_primer.xml"] {
        let mut fdm = load(repo_root, case)?;
        for _ in 0..1200 {
            fdm.run().map_err(port(case))?;
        }
        let mut snapshot = Snapshot::new();
        fdm.snapshot_into(&mut snapshot).map_err(port(case))?;
        let frame = mean_ns(N, || fdm.run().map(|_| ()));
        let frame = frame.map_err(port(case))?;
        let take = mean_ns(N, || fdm.snapshot_into(&mut snapshot)).map_err(port(case))?;
        let back = mean_ns(N, || fdm.restore(&snapshot)).map_err(port(case))?;
        info!(
            "{case}: frame {frame:.0} ns, snapshot_into {take:.0} ns, restore {back:.0} ns, {} bytes",
            snapshot.as_bytes().len()
        );
    }
    Ok(())
}
