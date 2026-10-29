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
//! allocates.

use std::path::{Path, PathBuf};

use jsbsim::FileResolver;
use jsbsim::fdm::Fdm;
use jsbsim::snapshot::Snapshot;
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
/// a snapshot, run, restore it and run again, after a first snapshot
/// that may allocate; fewer when the script ends.
#[cfg(test)]
fn measure_snapshots(fdm: &mut Fdm, frames: u32) -> (u64, u64) {
    let mut snapshot = Snapshot::new();
    fdm.snapshot_into(&mut snapshot).unwrap();
    let before = dhat::HeapStats::get();
    fdm.restore(&snapshot).unwrap();
    for _ in 0..frames {
        fdm.snapshot_into(&mut snapshot).unwrap();
        fdm.run().unwrap();
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
        let mut fdm = Fdm::new().unwrap();
        fdm.load_model(resolver, "aircraft", name).unwrap();
        for (property, value) in oracle::bench::AIRBORNE_IC {
            fdm.set_property(property, value).unwrap();
        }
        if let Some((_, missing)) = oracle::bench::UNRUNNABLE.iter().find(|(n, _)| n == name) {
            let err = fdm.run_ic().err();
            assert!(
                err.as_ref()
                    .is_some_and(|e| format!("{e:?}").contains(missing)),
                "{name} was expected to stop at {missing}: {err:?}"
            );
            continue;
        }
        fdm.run_ic().unwrap();
        report.push((
            format!("aircraft {name}"),
            FRAMES,
            measure(&mut fdm, FRAMES),
        ));
    }
}

/// The trimming scripts, measured from their first frame after the trim
/// has completed.
#[cfg(test)]
fn measure_trimmed(report: &mut Vec<(String, u32, (u64, u64))>) {
    for run in TRIMMED {
        let mut fdm = run.load(&oracle_dir()).unwrap();
        fdm.run_ic().unwrap();
        run_until_trimmed(&mut fdm, run.script);
        report.push((
            format!("trimmed script {}", run.script),
            run.frames,
            measure(&mut fdm, run.frames),
        ));
    }
}

/// The scripts of `oracle::steady_state::SCRIPTS`.
#[cfg(test)]
fn measure_scripts(report: &mut Vec<(String, u32, (u64, u64))>) {
    for run in SCRIPTS {
        let mut fdm = run.load(&oracle_dir()).unwrap();
        fdm.run_ic().unwrap();
        report.push((
            format!("script {}", run.script),
            run.frames,
            measure(&mut fdm, run.frames),
        ));
    }
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
        let mut fdm = Fdm::new().unwrap();
        fdm.set_systems_path(system.systems_path);
        fdm.load_model(&oracle_resolver, "derived/aircraft", &name)
            .unwrap();
        fdm.set_property("ic/h-sl-ft", 10000.0).unwrap();
        fdm.run_ic().unwrap();
        report.push((
            format!("aircraft {name}"),
            FRAMES,
            measure(&mut fdm, FRAMES),
        ));
    }

    // The planet tripods fly the Martian and NRLMSISE-00 atmospheres.
    for name in PLANET_TRIPODS {
        let mut fdm = Fdm::new().unwrap();
        fdm.load_model(&oracle_resolver, PLANET_TRIPOD_DIR, name)
            .unwrap();
        fdm.set_property("ic/h-sl-ft", PLANET_TRIPOD_ALTITUDE_FT)
            .unwrap();
        fdm.run_ic().unwrap();
        report.push((
            format!("aircraft {name}"),
            FRAMES,
            measure(&mut fdm, FRAMES),
        ));
    }

    measure_scripts(&mut report);
    measure_trimmed(&mut report);

    let failures: Vec<_> = report
        .iter()
        .filter(|(_, _, (blocks, bytes))| *blocks != 0 || *bytes != 0)
        .collect();
    assert!(
        failures.is_empty(),
        "allocations over the measured frames (what, frames, (blocks, bytes)): {report:?}"
    );
}
