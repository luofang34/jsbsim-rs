// SPDX-License-Identifier: LGPL-2.1-or-later
//! Steady-state frames allocate nothing, as JSBSim's `Run()` does not.
//!
//! Every aircraft that `oracle/model-loading.txt` records as loading `ok`
//! must be vendored under `oracle/inputs/aircraft/<name>/`; each is flown
//! from the airborne initial condition `oracle::bench::AIRBORNE_IC` (which
//! the bench's aircraft cases fly from too) unless it is one of
//! `oracle::bench::UNRUNNABLE`, and each vendored script that
//! stays airborne long enough runs through its events, while dhat counts
//! heap blocks across the measured frames. dhat allows one profiler per
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

/// (root under `oracle/`, script) of every script that trims through
/// `simulation/do_simple_trim`.
#[cfg(test)]
const TRIMMED: [(&str, &str); 10] = [
    ("inputs", "scripts/c1722.xml"),
    ("inputs", "scripts/c172_cruise_8K.xml"),
    ("inputs", "scripts/c172_cross_wind.xml"),
    ("inputs", "scripts/c172_head_wind.xml"),
    ("inputs", "scripts/c172_elevator_doublet.xml"),
    ("inputs", "scripts/p51d_tail_wind.xml"),
    ("inputs", "scripts/Short_S23_2.xml"),
    ("inputs", "scripts/Short_S23_3.xml"),
    ("inputs", "scripts/Short_S23_4.xml"),
    ("inputs", "scripts/737_cruise.xml"),
];

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
/// has completed: a trim allocates its axes and contacts when it runs, as
/// FGTrim does, and the frames after it nothing.
#[cfg(test)]
fn measure_trimmed(report: &mut Vec<(String, u32, (u64, u64))>) {
    for (root, script) in TRIMMED {
        let resolver = FileResolver::new(oracle_dir().join(root));
        let mut fdm = Fdm::new().unwrap();
        fdm.load_script(&resolver, script).unwrap();
        fdm.run_ic().unwrap();
        run_until_trimmed(&mut fdm, script);
        report.push((
            format!("trimmed script {script}"),
            FRAMES,
            measure(&mut fdm, FRAMES),
        ));
    }
}

/// The vendored, scenario and derived scripts, and the c172x without
/// sensor noise flown from the airborne initial condition.
#[cfg(test)]
fn measure_scripts(resolver: &FileResolver, report: &mut Vec<(String, u32, (u64, u64))>) {
    // (inputs, script, measured frames): ball_orbit_phase's window covers
    // its undock burn ramps at 10 s and 12 s; mk82's ends before touchdown;
    // gear_rover_taxi's rolls, steers, brakes and rests on every kind of
    // contact, and gear_rover_retract's slides on its skids with the gear
    // up and lands back on its wheels; piston_start_c310's cranks, starts,
    // runs on each magneto, cuts off and restarts its engines, and
    // piston_governor_c310's governs, feathers, windmills and restarts in
    // flight; c1722_inertia's trims, starts running from its initial
    // conditions and burns its tanks down to the unusable fuel; f16_test's
    // turbine spins up, lights and spools to full throttle; L4102's
    // turboprops start running from their initial conditions; and
    // Test_F450_Launch's brushless motors lift the quadcopter off.
    let scenarios = FileResolver::new(oracle_dir().join("scenarios"));
    let derived = FileResolver::new(oracle_dir().join("derived"));
    for (resolver, script, frames) in [
        (resolver, "scripts/ball_orbit_phase.xml", 3000),
        (resolver, "scripts/ball.xml", FRAMES),
        (resolver, "scripts/mk82_script.xml", FRAMES),
        (&scenarios, "scripts/gear_rover_taxi.xml", 2000),
        (&scenarios, "scripts/gear_rover_retract.xml", 1000),
        (&derived, "scripts/piston_start_c310.xml", 3000),
        (&derived, "scripts/piston_governor_c310.xml", 5000),
        (&derived, "scripts/c1722_inertia.xml", 12000),
        (resolver, "scripts/f16_test.xml", 3000),
        (resolver, "scripts/L4102.xml", 3000),
        (resolver, "scripts/Test_F450_Launch.xml", 3000),
    ] {
        let mut fdm = Fdm::new().unwrap();
        fdm.load_script(resolver, script).unwrap();
        fdm.run_ic().unwrap();
        report.push((
            format!("script {script}"),
            frames,
            measure(&mut fdm, frames),
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

    measure_scripts(&resolver, &mut report);
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
