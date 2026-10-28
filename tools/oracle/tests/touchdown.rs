// SPDX-License-Identifier: LGPL-2.1-or-later
//! Gear forces are not ported, so a compressed contact stops the run with
//! a typed error naming the contact and the time. When FGLGear's forces
//! are ported these tests flip: the runs must then go on.

use std::path::{Path, PathBuf};

use jsbsim::fdm::Fdm;
use jsbsim::{Error, FileResolver};

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
fn a_landing_cannonball_stops_at_touchdown() {
    let mut fdm = load("scripts/cannonball.xml");
    fdm.run_ic().unwrap();
    let stop = loop {
        match fdm.run() {
            Ok(true) => {}
            Ok(false) => panic!("the script ended before touchdown"),
            Err(e) => break e,
        }
    };
    assert_eq!(
        stop,
        Error::Touchdown {
            contact: "CONTACT".to_owned(),
            time: 36.98331853999749,
        }
    );
}

#[test]
fn a_contact_compressed_at_run_ic_fails_at_run_ic() {
    let mut fdm = load("scripts/cannonball.xml");
    fdm.set_property("ic/h-agl-ft", -1.0).unwrap();
    match fdm.run_ic() {
        Err(Error::Touchdown { contact, time }) => {
            assert_eq!(contact, "CONTACT");
            assert_eq!(time, 0.0);
        }
        other => panic!("expected a touchdown at RunIC, got {other:?}"),
    }
}
