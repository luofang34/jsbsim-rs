// SPDX-License-Identifier: LGPL-2.1-or-later
//! Steady-state frames allocate nothing, as JSBSim's `Run()` does not.
//!
//! Every aircraft that `oracle/model-loading.txt` records as loading `ok`
//! must be vendored under `oracle/inputs/aircraft/<name>/`; each is flown
//! from an airborne initial condition, and each vendored script that stays
//! airborne long enough runs through its events, while dhat counts heap
//! blocks across the measured frames. dhat allows one profiler per
//! process, so everything runs in one test of its own binary.

use std::path::{Path, PathBuf};

use jsbsim::FileResolver;
use jsbsim::fdm::Fdm;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// Frames run after `run_ic` before measuring, so one-time work (late
/// binding, the first event firings) is out of the window.
const WARM_UP: u32 = 100;
/// Measured frames for each aircraft.
const FRAMES: u32 = 1000;

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
        fdm.load_model(&resolver, "aircraft", name).unwrap();
        fdm.set_property("ic/h-sl-ft", 10000.0).unwrap();
        fdm.set_property("ic/u-fps", 500.0).unwrap();
        fdm.run_ic().unwrap();
        report.push((
            format!("aircraft {name}"),
            FRAMES,
            measure(&mut fdm, FRAMES),
        ));
    }

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

    // (script, measured frames): ball_orbit_phase's window covers its
    // undock burn ramps at 10 s and 12 s; mk82's ends before touchdown.
    for (script, frames) in [
        ("scripts/ball_orbit_phase.xml", 3000),
        ("scripts/ball.xml", FRAMES),
        ("scripts/mk82_script.xml", FRAMES),
    ] {
        let mut fdm = Fdm::new().unwrap();
        fdm.load_script(&resolver, script).unwrap();
        fdm.run_ic().unwrap();
        report.push((
            format!("script {script}"),
            frames,
            measure(&mut fdm, frames),
        ));
    }

    let failures: Vec<_> = report
        .iter()
        .filter(|(_, _, (blocks, bytes))| *blocks != 0 || *bytes != 0)
        .collect();
    assert!(
        failures.is_empty(),
        "allocations over the measured frames (what, frames, (blocks, bytes)): {report:?}"
    );
}
