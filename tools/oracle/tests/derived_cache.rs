// SPDX-License-Identifier: LGPL-2.1-or-later
//! Steady-state frames read no derived value (Euler angles, rotation
//! matrices, geodetic position) from a `Location` or `Quaternion` whose
//! owner forgot `cache_derived`: such a read recomputes the values on every
//! call. The library reports each such read as a trace event under
//! `jsbsim::math::UNCACHED_DERIVED_TARGET`; this test counts them over the
//! corpus aircraft that load `ok`, the vendored scripts and the derived
//! piston scripts.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use jsbsim::FileResolver;
use jsbsim::fdm::Fdm;
use jsbsim::math::UNCACHED_DERIVED_TARGET;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

#[cfg(test)]
const WARM_UP: u32 = 100;
#[cfg(test)]
const FRAMES: u32 = 1000;

#[cfg(test)]
struct Count(Arc<AtomicUsize>);

#[cfg(test)]
impl<S: tracing::Subscriber> Layer<S> for Count {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        if event.metadata().target() == UNCACHED_DERIVED_TARGET {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
fn oracle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

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

/// Uncached derived reads across the measured frames of `fdm`.
#[cfg(test)]
fn misses(fdm: &mut Fdm, count: &AtomicUsize, frames: u32) -> usize {
    for _ in 0..WARM_UP {
        assert!(fdm.run().unwrap());
    }
    let before = count.load(Ordering::Relaxed);
    for _ in 0..frames {
        assert!(fdm.run().unwrap());
    }
    count.load(Ordering::Relaxed) - before
}

#[test]
fn steady_state_frames_read_only_cached_derived_values() {
    let count = Arc::new(AtomicUsize::new(0));
    let subscriber = tracing_subscriber::registry().with(Count(Arc::clone(&count)));
    tracing::subscriber::with_default(subscriber, || {
        let inputs = oracle_dir().join("inputs");
        let resolver = FileResolver::new(&inputs);
        let mut report = Vec::new();
        for name in loading_aircraft() {
            let mut fdm = Fdm::new().unwrap();
            fdm.load_model(&resolver, "aircraft", &name).unwrap();
            fdm.set_property("ic/h-sl-ft", 10000.0).unwrap();
            fdm.set_property("ic/u-fps", 500.0).unwrap();
            if let Some((_, missing)) = oracle::bench::UNRUNNABLE.iter().find(|(n, _)| *n == name) {
                let err = fdm.run_ic().err();
                assert!(
                    err.as_ref()
                        .is_some_and(|e| format!("{e:?}").contains(missing)),
                    "{name} was expected to stop at {missing}: {err:?}"
                );
                continue;
            }
            fdm.run_ic().unwrap();
            report.push((format!("aircraft {name}"), misses(&mut fdm, &count, FRAMES)));
        }
        let derived = FileResolver::new(oracle_dir().join("derived"));
        for (resolver, script, frames) in [
            (&resolver, "scripts/ball_orbit_phase.xml", 3000),
            (&resolver, "scripts/ball.xml", FRAMES),
            (&resolver, "scripts/mk82_script.xml", FRAMES),
            (&resolver, "scripts/c1724.xml", FRAMES),
            (&derived, "scripts/piston_start_c310.xml", 3000),
            (&derived, "scripts/piston_governor_c310.xml", 5000),
            (&derived, "scripts/c1722_inertia.xml", 12000),
            (&resolver, "scripts/f16_test.xml", 3000),
            (&resolver, "scripts/L4102.xml", 3000),
            (&resolver, "scripts/Test_F450_Launch.xml", 3000),
        ] {
            let mut fdm = Fdm::new().unwrap();
            fdm.load_script(resolver, script).unwrap();
            fdm.run_ic().unwrap();
            report.push((format!("script {script}"), misses(&mut fdm, &count, frames)));
        }
        // The trimming scripts, from their first frame after the trim.
        for (root, script) in TRIMMED {
            let resolver = FileResolver::new(oracle_dir().join(root));
            let mut fdm = Fdm::new().unwrap();
            fdm.load_script(&resolver, script).unwrap();
            fdm.run_ic().unwrap();
            run_until_trimmed(&mut fdm, script);
            report.push((
                format!("trimmed script {script}"),
                misses(&mut fdm, &count, FRAMES),
            ));
        }
        assert!(
            report.iter().all(|(_, n)| *n == 0),
            "uncached derived reads over the measured frames: {report:?}"
        );
    });
}

#[test]
fn an_uncached_read_is_counted_and_a_cached_one_is_not() {
    let count = Arc::new(AtomicUsize::new(0));
    let subscriber = tracing_subscriber::registry().with(Count(Arc::clone(&count)));
    tracing::subscriber::with_default(subscriber, || {
        let mut location = jsbsim::math::Location::default();
        location.set_ellipse(20925646.32546, 20855486.595293265);
        location.set_position_geodetic(0.1, 0.5, 1000.0);
        let uncached = location.geod_latitude_rad();
        assert_eq!(count.load(Ordering::Relaxed), 1);
        location.cache_derived();
        assert_eq!(location.geod_latitude_rad().to_bits(), uncached.to_bits());
        assert_eq!(count.load(Ordering::Relaxed), 1);
    });
}
