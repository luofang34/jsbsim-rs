// SPDX-License-Identifier: LGPL-2.1-or-later
//! Steady-state frames allocate nothing, as JSBSim's `Run()` does not.
//!
//! Every aircraft that `oracle/model-loading.txt` records as loading `ok`
//! must be vendored under `oracle/inputs/aircraft/<name>/`; each is flown
//! from the airborne initial condition `oracle::bench::AIRBORNE_IC` (which
//! the bench's aircraft cases fly from too) unless it is one of
//! `oracle::bench::UNRUNNABLE`, and each script of
//! `oracle::steady_state` runs through its events, while dhat counts
//! heap blocks across the measured frames. dhat allows one profiler per
//! process, so everything runs in one test of its own binary.
//!
//! Each run then takes a snapshot and restores it every frame, as a host
//! rewinding frame by frame does: after the first, neither
//! `Fdm::snapshot_into` nor `Fdm::restore` on a reused `Snapshot`
//! allocates. Before each restore, the first snapshot with one byte
//! appended is restored: the check that precedes every restore decodes
//! all of it, of a state frames older, before it refuses the extra byte,
//! so that check alone is measured too.
//!
//! Reading and writing existing properties by name, as a host bridging
//! controls and sensors each frame does, allocates nothing either.

use std::path::{Path, PathBuf};

use jsbsim::Fdm;
use jsbsim::snapshot::{Snapshot, SnapshotError};
use jsbsim::{Error, FileResolver};
use oracle::steady_state::{
    FRAMES, PLANET_TRIPOD_ALTITUDE_FT, PLANET_TRIPOD_DIR, PLANET_TRIPODS, SCRIPTS, TRIMMED,
};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// Frames run after `run_ic` before measuring, so one-time work (late
/// binding, the first event firings) is out of the window.
const WARM_UP: u32 = 100;

#[cfg(test)]
fn oracle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

/// Aircraft whose recorded load outcome is `ok`.
#[cfg(test)]
fn loading_aircraft() -> Vec<String> {
    let text = std::fs::read_to_string(oracle_dir().join("model-loading.txt")).unwrap();
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once(' '))
        .filter(|(_, outcome)| outcome.trim() == "ok")
        .map(|(name, _)| name.to_owned())
        .collect()
}

/// Heap blocks and bytes allocated across `frames` frames of `fdm`.
#[cfg(test)]
fn measure(fdm: &mut Fdm, frames: u32) -> (u64, u64) {
    for _ in 0..WARM_UP {
        assert!(fdm.run().unwrap(), "the run ended during warm-up");
    }
    let before = dhat::HeapStats::get();
    for _ in 0..frames {
        assert!(fdm.run().unwrap(), "the run ended while measured");
    }
    let after = dhat::HeapStats::get();
    let (run_blocks, run_bytes) = (
        after.total_blocks - before.total_blocks,
        after.total_bytes - before.total_bytes,
    );
    let (snapshot_blocks, snapshot_bytes) = measure_snapshots(fdm, frames.min(SNAPSHOT_FRAMES));
    (run_blocks + snapshot_blocks, run_bytes + snapshot_bytes)
}

/// Frames measured with a snapshot and a restore each.
const SNAPSHOT_FRAMES: u32 = 200;

/// Heap blocks and bytes allocated across `frames` frames that each take
/// a snapshot, run, check the first snapshot, restore the one taken and
/// run again, after a first snapshot that may allocate; fewer when the script
/// ends.
#[cfg(test)]
fn measure_snapshots(fdm: &mut Fdm, frames: u32) -> (u64, u64) {
    let mut snapshot = Snapshot::new();
    fdm.snapshot_into(&mut snapshot).unwrap();
    let mut padded = snapshot.as_bytes().to_vec();
    padded.push(0);
    let checked_only = Snapshot::from_bytes(&padded).unwrap();
    let trailing = Err(Error::Snapshot(SnapshotError::TrailingBytes { extra: 1 }));
    let before = dhat::HeapStats::get();
    fdm.restore(&snapshot).unwrap();
    for _ in 0..frames {
        fdm.snapshot_into(&mut snapshot).unwrap();
        fdm.run().unwrap();
        assert_eq!(fdm.restore(&checked_only), trailing);
        fdm.restore(&snapshot).unwrap();
        if !fdm.run().unwrap() {
            break;
        }
    }
    let after = dhat::HeapStats::get();
    (
        after.total_blocks - before.total_blocks,
        after.total_bytes - before.total_bytes,
    )
}

/// Records what `case` (load, start and [`measure`] one run) allocates.
/// dhat counts the allocations of every thread, and the test harness's
/// own thread allocates once when it notes that a test has run for 60
/// seconds, so a count that is not zero is measured again from a fresh
/// load: the port is deterministic, so what it allocates itself recurs,
/// while the harness's note does not.
#[cfg(test)]
fn record(
    report: &mut Vec<(String, u32, (u64, u64))>,
    what: String,
    frames: u32,
    case: impl Fn() -> (u64, u64),
) {
    let mut counts = case();
    if counts != (0, 0) {
        counts = case();
    }
    report.push((what, frames, counts));
}

/// Runs `fdm` until its script's trim has completed.
#[cfg(test)]
fn run_until_trimmed(fdm: &mut Fdm, script: &str) {
    for _ in 0..5000 {
        if fdm.get_property("simulation/trim-completed").unwrap() == 1.0 {
            return;
        }
        assert!(fdm.run().unwrap(), "{script} ended before its trim");
    }
    panic!("{script} did not trim within 5000 frames");
}

/// The corpus aircraft that load `ok`, flown from the airborne initial
/// condition.
#[cfg(test)]
fn measure_corpus(
    inputs: &Path,
    resolver: &FileResolver,
    report: &mut Vec<(String, u32, (u64, u64))>,
) {
    let aircraft = loading_aircraft();
    assert!(!aircraft.is_empty(), "no aircraft loads ok");
    for name in &aircraft {
        let file = inputs
            .join("aircraft")
            .join(name)
            .join(format!("{name}.xml"));
        assert!(
            file.is_file(),
            "{name} loads ok but is not vendored at {}",
            file.display()
        );
        let airborne = || {
            let mut fdm = Fdm::new().unwrap();
            fdm.load_model(resolver, "aircraft", name).unwrap();
            for (property, value) in oracle::bench::AIRBORNE_IC {
                fdm.set_property(property, value).unwrap();
            }
            fdm
        };
        if let Some((_, missing)) = oracle::bench::UNRUNNABLE.iter().find(|(n, _)| n == name) {
            let err = airborne().run_ic().err();
            assert!(
                err.as_ref()
                    .is_some_and(|e| format!("{e:?}").contains(missing)),
                "{name} was expected to stop at {missing}: {err:?}"
            );
            continue;
        }
        record(report, format!("aircraft {name}"), FRAMES, || {
            let mut fdm = airborne();
            fdm.run_ic().unwrap();
            measure(&mut fdm, FRAMES)
        });
    }
}

/// The trimming scripts, measured from their first frame after the trim
/// has completed.
#[cfg(test)]
fn measure_trimmed(report: &mut Vec<(String, u32, (u64, u64))>) {
    for run in TRIMMED {
        record(
            report,
            format!("trimmed script {}", run.script),
            run.frames,
            || {
                let mut fdm = run.load(&oracle_dir()).unwrap();
                fdm.run_ic().unwrap();
                run_until_trimmed(&mut fdm, run.script);
                measure(&mut fdm, run.frames)
            },
        );
    }
}

/// The scripts of `oracle::steady_state::SCRIPTS`.
#[cfg(test)]
fn measure_scripts(report: &mut Vec<(String, u32, (u64, u64))>) {
    for run in SCRIPTS {
        record(report, format!("script {}", run.script), run.frames, || {
            let mut fdm = run.load(&oracle_dir()).unwrap();
            fdm.run_ic().unwrap();
            measure(&mut fdm, run.frames)
        });
    }
}

/// Properties a host reads or writes by name every frame, in the forms a
/// name takes: plain, indexed, from the root and through `..`.
const BY_NAME: [&str; 4] = [
    "velocities/vc-kts",
    "fcs/throttle-cmd-norm[0]",
    "/position/h-sl-ft",
    "fcs/../fcs/elevator-cmd-norm",
];

/// Heap blocks and bytes allocated across `frames` frames of the c172x
/// that each read and write the [`BY_NAME`] properties by name, after
/// warm-up frames that do so too (a first write may set up its setter's
/// state once, as the altitude's does).
#[cfg(test)]
fn measure_by_name(resolver: &FileResolver, frames: u32) -> (u64, u64) {
    let mut fdm = Fdm::new().unwrap();
    fdm.load_model(resolver, "aircraft", "c172x").unwrap();
    for (property, value) in oracle::bench::AIRBORNE_IC {
        fdm.set_property(property, value).unwrap();
    }
    fdm.run_ic().unwrap();
    let frame = |fdm: &mut Fdm| {
        for name in BY_NAME {
            let value = fdm.get_property(name).unwrap();
            fdm.set_property(name, value).unwrap();
        }
        fdm.run().unwrap();
    };
    for _ in 0..WARM_UP {
        frame(&mut fdm);
    }
    let before = dhat::HeapStats::get();
    for _ in 0..frames {
        frame(&mut fdm);
    }
    let after = dhat::HeapStats::get();
    (
        after.total_blocks - before.total_blocks,
        after.total_bytes - before.total_bytes,
    )
}

#[test]
fn steady_state_frames_allocate_nothing() {
    let _profiler = dhat::Profiler::builder().testing().build();
    let inputs = oracle_dir().join("inputs");
    let resolver = FileResolver::new(&inputs);
    let mut report = Vec::new();

    measure_corpus(&inputs, &resolver, &mut report);

    // The derived tripods run every ported FCS component family.
    let oracle_resolver = FileResolver::new(oracle_dir());
    for system in oracle::derived::tripod::SYSTEMS.iter().filter(|s| s.loads) {
        let name = format!("tripod_{}", system.suffix);
        record(&mut report, format!("aircraft {name}"), FRAMES, || {
            let mut fdm = Fdm::new().unwrap();
            fdm.set_systems_path(system.systems_path);
            fdm.load_model(&oracle_resolver, "derived/aircraft", &name)
                .unwrap();
            fdm.set_property("ic/h-sl-ft", 10000.0).unwrap();
            fdm.run_ic().unwrap();
            measure(&mut fdm, FRAMES)
        });
    }

    // The planet tripods fly the Martian and NRLMSISE-00 atmospheres.
    for name in PLANET_TRIPODS {
        record(&mut report, format!("aircraft {name}"), FRAMES, || {
            let mut fdm = Fdm::new().unwrap();
            fdm.load_model(&oracle_resolver, PLANET_TRIPOD_DIR, name)
                .unwrap();
            fdm.set_property("ic/h-sl-ft", PLANET_TRIPOD_ALTITUDE_FT)
                .unwrap();
            fdm.run_ic().unwrap();
            measure(&mut fdm, FRAMES)
        });
    }

    measure_scripts(&mut report);
    measure_trimmed(&mut report);
    record(&mut report, "properties by name".to_owned(), FRAMES, || {
        measure_by_name(&resolver, FRAMES)
    });

    assert!(
        !report
            .iter()
            .any(|(_, _, (blocks, bytes))| *blocks != 0 || *bytes != 0),
        "allocations over the measured frames (what, frames, (blocks, bytes)): {report:?}"
    );
}
