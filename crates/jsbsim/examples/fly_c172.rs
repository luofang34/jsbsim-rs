// SPDX-License-Identifier: LGPL-2.1-or-later
//! Runs JSBSim's `scripts/c1723.xml`: a Cessna 172 takes off on autopilot
//! and climbs. Reads a JSBSim data tree from disk: `$JSBSIM_ROOT`, or the
//! copy of JSBSim's files this repository keeps in `oracle/inputs`.
//!
//! `cargo run -p jsbsim --features fs --release --example fly_c172`

use std::path::PathBuf;

use jsbsim::{Fdm, FileResolver};

// ANCHOR: root
/// The JSBSim data tree: a directory holding `aircraft/`, `engine/`,
/// `systems/` and `scripts/`, such as a checkout of JSBSim itself.
fn data_root() -> PathBuf {
    std::env::var_os("JSBSIM_ROOT").map_or_else(
        || PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../oracle/inputs")),
        PathBuf::from,
    )
}
// ANCHOR_END: root

fn main() -> Result<(), jsbsim::Error> {
    tracing_subscriber::fmt().try_init().ok();

    // ANCHOR: script
    let files = FileResolver::new(data_root());
    let mut fdm = Fdm::new()?;
    // Loads the aircraft and the initialization file the script names,
    // its properties and its events. The script fixes the time step.
    fdm.load_script(&files, "scripts/c1723.xml")?;
    fdm.run_ic()?;

    let altitude = fdm.node("position/h-agl-ft")?;
    let mut highest: f64 = 0.0;
    // `run` returns false once the script's end time has passed.
    while fdm.run()? {
        if let Some(altitude) = altitude {
            highest = highest.max(fdm.read_node(altitude)?);
        }
    }
    tracing::info!(
        "script ended at {:.1} s; highest {highest:.0} ft above ground",
        fdm.sim_time()
    );
    // ANCHOR_END: script

    assert!(fdm.sim_time() >= 200.0);
    assert!(highest > 500.0, "highest {highest}");
    Ok(())
}

#[test]
fn runs() -> Result<(), jsbsim::Error> {
    main()
}
