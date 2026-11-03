// SPDX-License-Identifier: LGPL-2.1-or-later
//! Trims a Cessna 172 for level flight, flies the trimmed state, and
//! linearizes about it. Reads the JSBSim data tree at `$JSBSIM_ROOT`, or
//! this repository's `oracle/inputs`.
//!
//! `cargo run -p jsbsim --features fs --release --example trim_and_linearize`

use std::path::PathBuf;

use jsbsim::trim::{Control, State};
use jsbsim::{Error, Fdm, FileResolver, TrimMode};

fn data_root() -> PathBuf {
    std::env::var_os("JSBSIM_ROOT").map_or_else(
        || PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../oracle/inputs")),
        PathBuf::from,
    )
}

fn airborne_c172() -> Result<Fdm, Error> {
    let files = FileResolver::new(data_root());
    let mut fdm = Fdm::new()?;
    fdm.load_model(&files, "aircraft", "c172x")?;
    fdm.load_initial_conditions(&files, "aircraft/c172x", "reset01")?;
    fdm.run_ic()?;
    Ok(fdm)
}

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().try_init().ok();
    let mut fdm = airborne_c172()?;

    // ANCHOR: trim
    let report = fdm.do_trim(TrimMode::Full)?;
    for axis in &report.axes {
        tracing::info!(
            "{:>6} by {:<16} -> {:+.6} (state {:+.2e}, tolerance {:.0e})",
            axis.state.name(),
            axis.control.name(),
            axis.control_value,
            axis.state_value,
            axis.tolerance,
        );
    }
    // ANCHOR_END: trim
    assert!(report.success);

    // ANCHOR: fly_trimmed
    let start = fdm.get_property("position/h-sl-ft")?;
    while fdm.sim_time() < 10.0 {
        fdm.run()?;
    }
    let drift = fdm.get_property("position/h-sl-ft")? - start;
    tracing::info!("altitude change over 10 s trimmed: {drift:+.2} ft");
    // ANCHOR_END: fly_trimmed
    assert!(drift.abs() < 20.0, "drift {drift}");

    // ANCHOR: linearize
    let lin = fdm.do_linearization()?;
    tracing::info!("states  {:?}", lin.x_names());
    tracing::info!("inputs  {:?}", lin.u_names());
    let (rows, cols) = lin.a.shape();
    tracing::info!("A is {rows}x{cols}, B is {:?}", lin.b.shape());
    // How the pitch rate responds to the elevator: B[Q][DeCmd].
    let q = lin.x_names().iter().position(|&n| n == "Q");
    let de = lin.u_names().iter().position(|&n| n == "DeCmd");
    if let (Some(q), Some(de)) = (q, de) {
        tracing::info!("dQ/dt per unit elevator: {:?} rad/s^2", lin.b.get(q, de));
    }
    // ANCHOR_END: linearize
    assert_eq!(lin.u_names(), ["ThtlCmd", "DaCmd", "DeCmd", "DrCmd"]);
    assert_eq!(rows, cols);

    // ANCHOR: custom
    // A custom trim names each axis: the state to zero and the control
    // that zeroes it, solved in order. These three are the longitudinal
    // mode's own.
    let mut custom = airborne_c172()?;
    let report = custom.do_trim_custom(&[
        (State::Wdot, Control::Alpha),
        (State::Udot, Control::Throttle),
        (State::Qdot, Control::PitchTrim),
    ])?;
    tracing::info!(
        "custom trim converged in {} cycles",
        report.total_iterations
    );
    let custom_throttle = custom.get_property("fcs/throttle-cmd-norm")?;

    let mut longitudinal = airborne_c172()?;
    longitudinal.do_trim(TrimMode::Longitudinal)?;
    let throttle = longitudinal.get_property("fcs/throttle-cmd-norm")?;
    assert_eq!(custom_throttle, throttle);
    // ANCHOR_END: custom
    Ok(())
}

#[test]
fn runs() -> Result<(), Error> {
    main()
}
