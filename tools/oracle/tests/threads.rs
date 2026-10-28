// SPDX-License-Identifier: LGPL-2.1-or-later
//! An FDM can move to a worker thread, and the math value types are `Copy`:
//! a simulation run on a spawned thread produces, bit for bit, what it
//! produces on the main thread.

use std::path::{Path, PathBuf};

use jsbsim::FileResolver;
use jsbsim::fdm::Fdm;
use jsbsim::math::{ColumnVector3, Location, Matrix33, Quaternion};

#[cfg(test)]
const PROPERTIES: [&str; 8] = [
    "position/lat-geod-rad",
    "position/long-gc-rad",
    "position/h-sl-ft",
    "velocities/u-fps",
    "velocities/w-fps",
    "attitude/theta-rad",
    "aero/qbar-psf",
    "simulation/sim-time-sec",
];
#[cfg(test)]
const FRAMES: usize = 1500;

#[cfg(test)]
fn assert_send<T: Send>() {}

#[cfg(test)]
fn assert_copy<T: Copy>() {}

#[cfg(test)]
fn start(script: &str) -> Fdm {
    let inputs: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/inputs");
    let mut fdm = Fdm::new().unwrap();
    fdm.load_script(&FileResolver::new(inputs), script).unwrap();
    fdm.run_ic().unwrap();
    fdm
}

#[cfg(test)]
fn run(mut fdm: Fdm) -> Vec<u64> {
    let mut out = Vec::with_capacity(FRAMES * PROPERTIES.len());
    for _ in 0..FRAMES {
        fdm.run().unwrap();
        for p in PROPERTIES {
            out.push(fdm.get_property(p).unwrap().to_bits());
        }
    }
    out
}

#[test]
fn fdm_is_send_and_math_values_are_copy() {
    assert_send::<Fdm>();
    assert_copy::<Quaternion>();
    assert_copy::<Location>();
    assert_copy::<ColumnVector3>();
    assert_copy::<Matrix33>();
}

#[test]
fn a_run_on_a_spawned_thread_matches_the_main_thread_bit_for_bit() {
    for script in ["scripts/ball_orbit_phase.xml", "scripts/mk82_script.xml"] {
        let here = run(start(script));
        let fdm = start(script);
        let there = std::thread::spawn(move || run(fdm)).join().unwrap();
        assert_eq!(here, there, "{script}");
    }
}
