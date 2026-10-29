// SPDX-License-Identifier: LGPL-2.1-or-later
//! JSBSim's `tests/TestLinearization.py` on the 737 cruise script, through
//! the public API and through `simulation/do_linearization`; and the
//! executive property JSBSim does not tie, `simulation/do_simplex_trim`,
//! which only creates a plain property.

use std::path::{Path, PathBuf};

use jsbsim::FileResolver;
use jsbsim::fdm::Fdm;

#[cfg(test)]
fn inputs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/inputs")
}

/// The 737 cruise started, its engines running and one frame flown, as
/// the upstream test prepares it before trimming.
#[cfg(test)]
fn cruise() -> Fdm {
    let mut fdm = Fdm::new().unwrap();
    fdm.load_script(&FileResolver::new(inputs()), "scripts/737_cruise.xml")
        .unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("propulsion/engine[0]/set-running", 1.0)
        .unwrap();
    fdm.set_property("propulsion/engine[1]/set-running", 1.0)
        .unwrap();
    fdm.run().unwrap();
    fdm
}

#[cfg(test)]
const STATES: [&str; 12] = [
    "Vt",
    "Alpha",
    "Theta",
    "Q",
    "Beta",
    "Phi",
    "P",
    "Psi",
    "R",
    "Latitude",
    "Longitude",
    "Alt",
];

#[cfg(test)]
const STATE_UNITS: [&str; 12] = [
    "ft/s", "rad", "rad", "rad/s", "rad", "rad", "rad/s", "rad", "rad/s", "rad", "rad", "ft",
];

#[test]
fn the_trimmed_737_linearizes_into_twelve_states_and_four_inputs() {
    let mut fdm = cruise();
    fdm.set_property("simulation/do_simple_trim", 1.0).unwrap();
    let lin = fdm.linearize().unwrap();
    assert_eq!(lin.x0.len(), 12);
    assert_eq!(lin.u0.len(), 4);
    assert_eq!(lin.y0.len(), 12);
    let shape = |m: &Vec<Vec<f64>>| (m.len(), m.first().map_or(0, Vec::len));
    assert_eq!(shape(&lin.a), (12, 12));
    assert_eq!(shape(&lin.b), (12, 4));
    assert_eq!(shape(&lin.c), (12, 12));
    assert_eq!(shape(&lin.d), (12, 4));
    assert_eq!(lin.x_names(), STATES);
    assert_eq!(lin.y_names(), STATES);
    assert_eq!(lin.u_names(), ["ThtlCmd", "DaCmd", "DeCmd", "DrCmd"]);
    assert_eq!(lin.x_units(), STATE_UNITS);
    assert_eq!(lin.y_units(), STATE_UNITS);
    assert_eq!(lin.u_units(), ["norm"; 4]);
    // FGLinearization leaves the time step it suspended at zero.
    assert_eq!(fdm.get_property("simulation/dt").unwrap(), 0.0);
}

#[test]
fn do_linearization_keeps_the_result_and_restores_the_time_step() {
    let mut fdm = cruise();
    fdm.set_property("simulation/do_simple_trim", 1.0).unwrap();
    let dt = fdm.get_property("simulation/dt").unwrap();
    assert!(fdm.last_linearization().is_none());
    fdm.set_property("simulation/do_linearization", 1.0)
        .unwrap();
    let lin = fdm.last_linearization().unwrap();
    assert_eq!(lin.aircraft_name, "737");
    assert_eq!(lin.a.len(), 12);
    assert_eq!(fdm.get_property("simulation/dt").unwrap(), dt);
    let t = fdm.get_property("simulation/sim-time-sec").unwrap();
    fdm.run().unwrap();
    assert_eq!(fdm.get_property("simulation/sim-time-sec").unwrap(), t + dt);
}

/// Every property of the state a frame produces, by bits.
#[cfg(test)]
fn state(fdm: &mut Fdm) -> Vec<u64> {
    [
        "simulation/sim-time-sec",
        "position/h-sl-ft",
        "position/lat-geod-deg",
        "position/long-gc-deg",
        "velocities/u-fps",
        "velocities/v-fps",
        "velocities/w-fps",
        "velocities/p-rad_sec",
        "velocities/q-rad_sec",
        "velocities/r-rad_sec",
        "attitude/phi-rad",
        "attitude/theta-rad",
        "attitude/psi-rad",
        "accelerations/udot-ft_sec2",
        "accelerations/wdot-ft_sec2",
        "propulsion/engine[0]/thrust-lbs",
        "fcs/elevator-pos-rad",
        "simulation/trim-completed",
    ]
    .iter()
    .map(|p| fdm.get_property(p).unwrap().to_bits())
    .collect()
}

#[test]
fn do_simplex_trim_is_not_tied_and_changes_nothing() {
    let (mut plain, mut written) = (cruise(), cruise());
    assert!(
        written
            .node("simulation/do_simplex_trim")
            .unwrap()
            .is_none()
    );
    assert!(
        written
            .set_property("simulation/do_simplex_trim", 1.0)
            .unwrap()
    );
    assert_eq!(
        written.get_property("simulation/do_simplex_trim").unwrap(),
        1.0
    );
    for _ in 0..120 {
        plain.run().unwrap();
        written.run().unwrap();
        assert_eq!(state(&mut plain), state(&mut written));
    }
}
