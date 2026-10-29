// SPDX-License-Identifier: LGPL-2.1-or-later
//! A run goes on through touchdown: contacts compress, push back and let
//! the script reach its end.

use std::path::{Path, PathBuf};

use jsbsim::FileResolver;
use jsbsim::fdm::Fdm;

#[cfg(test)]
fn inputs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/inputs")
}

#[cfg(test)]
fn load(script: &str) -> Fdm {
    let mut fdm = Fdm::new().unwrap();
    fdm.load_script(&FileResolver::new(inputs()), script)
        .unwrap();
    fdm
}

#[test]
fn a_landing_cannonball_bounces_and_the_script_runs_to_its_end() {
    let mut fdm = load("scripts/cannonball.xml");
    fdm.run_ic().unwrap();
    // (time, WOW) at each change of the contact's WOW.
    let mut changes = Vec::new();
    let mut wow = false;
    while fdm.run().unwrap() {
        let now = fdm.get_property("gear/unit/WOW").unwrap() != 0.0;
        if now != wow {
            changes.push((fdm.get_property("simulation/sim-time-sec").unwrap(), now));
            wow = now;
        }
    }
    // The stiff contact throws the ball back up within two frames, as in
    // JSBSim; its reference run then lands again at t=217 s.
    assert_eq!(changes.first(), Some(&(36.98331853999749, true)));
    assert_eq!(changes.get(1).map(|&(_, w)| w), Some(false));
    assert!(changes[1].0 - 36.98331853999749 < 0.02);
    assert!(fdm.get_property("simulation/sim-time-sec").unwrap() > 799.9);
}

#[test]
fn a_contact_compressed_at_run_ic_carries_the_vehicle_from_the_start() {
    let mut fdm = load("scripts/cannonball.xml");
    fdm.set_property("ic/h-agl-ft", -1.0).unwrap();
    fdm.run_ic().unwrap();
    assert_eq!(fdm.get_property("simulation/sim-time-sec").unwrap(), 0.0);
    assert_eq!(fdm.get_property("gear/unit/WOW").unwrap(), 1.0);
    assert!(fdm.get_property("gear/unit/compression-ft").unwrap() > 0.9);
    // RunIC steps with dt = 0, where no compression speed is computed.
    assert_eq!(
        fdm.get_property("gear/unit/compression-velocity-fps")
            .unwrap(),
        0.0
    );
    assert!(fdm.run().unwrap());
}
