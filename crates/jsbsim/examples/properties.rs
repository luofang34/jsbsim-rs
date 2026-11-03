// SPDX-License-Identifier: LGPL-2.1-or-later
//! Flies a Cessna 172 by writing its control properties: by name for
//! one-off settings, by `NodeId` in the per-frame loop. Reads the JSBSim
//! data tree at `$JSBSIM_ROOT`, or this repository's `oracle/inputs`.
//!
//! `cargo run -p jsbsim --features fs --example properties`

use std::path::PathBuf;

use jsbsim::{Error, Fdm, FileResolver, TrimMode};

fn data_root() -> PathBuf {
    std::env::var_os("JSBSIM_ROOT").map_or_else(
        || PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../oracle/inputs")),
        PathBuf::from,
    )
}

// ANCHOR: airborne
/// The c172x at 4000 ft and 100 kt, its engine running (`reset01`).
fn airborne_c172() -> Result<Fdm, Error> {
    let files = FileResolver::new(data_root());
    let mut fdm = Fdm::new()?;
    fdm.load_model(&files, "aircraft", "c172x")?;
    fdm.load_initial_conditions(&files, "aircraft/c172x", "reset01")?;
    fdm.run_ic()?;
    Ok(fdm)
}
// ANCHOR_END: airborne

type Result<T, E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

/// The node at `path`, which the loaded model must have bound.
fn bound(fdm: &Fdm, path: &str) -> Result<jsbsim::NodeId> {
    Ok(fdm
        .node(path)?
        .ok_or_else(|| format!("the model binds no {path}"))?)
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().try_init().ok();
    let mut fdm = airborne_c172()?;

    // ANCHOR: by_name
    fdm.set_property("fcs/throttle-cmd-norm", 0.8)?;
    fdm.set_property("fcs/mixture-cmd-norm", 1.0)?;
    let speed = fdm.get_property("velocities/vc-kts")?;
    // ANCHOR_END: by_name
    tracing::info!("calibrated airspeed {speed:.1} kt");

    // ANCHOR: by_node
    fdm.do_trim(TrimMode::Full)?;
    // Look each path up once; a NodeId reads and writes without parsing
    // or searching the path again.
    let throttle = bound(&fdm, "fcs/throttle-cmd-norm")?;
    let climb_rate = bound(&fdm, "velocities/h-dot-fps")?;
    let trimmed = fdm.read_node(throttle)?;
    let mut fastest_climb: f64 = 0.0;
    for frame in 0..(10 * 120) {
        // Open the throttle from its trimmed setting to full over 5 s.
        let ramp = (f64::from(frame) / (5.0 * 120.0)).min(1.0);
        fdm.write_node(throttle, trimmed + (1.0 - trimmed) * ramp)?;
        fdm.run()?;
        fastest_climb = fastest_climb.max(fdm.read_node(climb_rate)?);
    }
    // ANCHOR_END: by_node
    tracing::info!("throttle {trimmed:.2} -> 1.00: climbing up to {fastest_climb:.1} ft/s");
    assert!(fastest_climb > 5.0, "climb {fastest_climb}");

    // ANCHOR: semantics
    // A path no model bound reads 0 and has no node...
    assert_eq!(fdm.get_property("no/such-property")?, 0.0);
    assert!(fdm.node("no/such-property")?.is_none());
    // ...until a write creates it.
    assert!(fdm.set_property("my/setting", 3.0)?);
    assert_eq!(fdm.get_property("my/setting")?, 3.0);
    // A read-only node ignores the write and says so.
    assert!(!fdm.set_property("simulation/sim-time-sec", 0.0)?);
    // ANCHOR_END: semantics

    // ANCHOR: hold
    // Pausing holds time and the models; run() still returns true.
    let t = fdm.sim_time();
    fdm.set_property("simulation/pause", 1.0)?;
    fdm.run()?;
    assert_eq!(fdm.sim_time(), t);
    fdm.set_property("simulation/pause", 0.0)?;
    // ANCHOR_END: hold
    Ok(())
}

#[test]
fn runs() -> Result<()> {
    main()
}
