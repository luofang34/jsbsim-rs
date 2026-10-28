// SPDX-License-Identifier: LGPL-2.1-or-later
//! Independent simulations in one process: two FDMs stepped alternately
//! produce, bit for bit, what each produces alone.

use std::path::{Path, PathBuf};

use jsbsim::FileResolver;
use jsbsim::fdm::Fdm;

#[cfg(test)]
const PROPERTIES: [&str; 8] = [
    "position/eci-x-ft",
    "position/eci-y-ft",
    "position/eci-z-ft",
    "velocities/u-fps",
    "velocities/w-fps",
    "attitude/theta-rad",
    "aero/qbar-psf",
    "simulation/sim-time-sec",
];
#[cfg(test)]
const FRAMES: usize = 1500;

#[cfg(test)]
fn inputs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/inputs")
}

#[cfg(test)]
fn start(script: &str) -> Fdm {
    let mut fdm = Fdm::new().unwrap();
    fdm.load_script(&FileResolver::new(inputs()), script)
        .unwrap();
    fdm.run_ic().unwrap();
    fdm
}

#[cfg(test)]
fn sample(fdm: &mut Fdm) -> Vec<u64> {
    PROPERTIES
        .iter()
        .map(|p| fdm.get_property(p).unwrap().to_bits())
        .collect()
}

#[cfg(test)]
fn solo(script: &str) -> Vec<Vec<u64>> {
    let mut fdm = start(script);
    (0..FRAMES)
        .map(|_| {
            fdm.run().unwrap();
            sample(&mut fdm)
        })
        .collect()
}

#[test]
fn interleaved_instances_match_their_solo_runs_bit_for_bit() {
    let scripts = ["scripts/ball_orbit_phase.xml", "scripts/cannonball.xml"];
    let alone: Vec<_> = scripts.iter().map(|s| solo(s)).collect();
    let mut fdms: Vec<Fdm> = scripts.iter().map(|s| start(s)).collect();
    for (frame, expected) in alone[0].iter().zip(&alone[1]).enumerate() {
        for ((fdm, want), script) in fdms.iter_mut().zip([expected.0, expected.1]).zip(scripts) {
            fdm.run().unwrap();
            assert_eq!(&sample(fdm), want, "{script} frame {frame}");
        }
    }
    assert_ne!(alone[0], alone[1]);
}
