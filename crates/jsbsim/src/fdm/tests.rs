// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

#[test]
fn construction_binds_model_and_exec_properties() {
    let mut fdm = Fdm::new().unwrap();
    for path in [
        "simulation/models/FGPropagate/enabled",
        "position/h-sl-ft",
        "accelerations/udot-ft_sec2",
        "metrics/Sw-sqft",
        "simulation/sim-time-sec",
        "forces/hold-down",
    ] {
        assert!(fdm.node(path).unwrap().is_some(), "{path}");
    }
    assert_eq!(
        fdm.get_property("simulation/models/FGPropagate/enabled")
            .unwrap(),
        1.0
    );
    assert_eq!(
        fdm.get_property("simulation/integrator/position/translational")
            .unwrap(),
        4.0
    );
}

#[test]
fn getters_are_live_and_read_only_nodes_reject_writes() {
    let mut fdm = Fdm::new().unwrap();
    assert!(fdm.set_property("metrics/Sw-sqft", 174.0).unwrap());
    assert_eq!(fdm.get_property("metrics/Sw-sqft").unwrap(), 174.0);
    assert!(
        !fdm.set_property("metrics/bw-ft", 1.0).unwrap(),
        "getter-only"
    );
    assert!(!fdm.set_property("simulation/sim-time-sec", 1.0).unwrap());
}

#[test]
fn time_advances_only_when_not_holding() {
    let mut fdm = Fdm::new().unwrap();
    // No aircraft is loaded, so the aerodynamics have no axis system and
    // would refuse to run, as in JSBSim.
    fdm.set_property("simulation/models/FGAerodynamics/enabled", 0.0)
        .unwrap();
    fdm.set_dt(0.5);
    fdm.run().unwrap();
    assert_eq!(fdm.get_property("simulation/sim-time-sec").unwrap(), 0.5);
    fdm.set_property("simulation/pause", 1.0).unwrap();
    fdm.run().unwrap();
    assert_eq!(fdm.get_property("simulation/frame").unwrap(), 1.0);
    fdm.set_property("simulation/terminate", 1.0).unwrap();
    assert!(!fdm.run().unwrap());
}

#[test]
fn model_rates_skip_frames_like_fgmodel() {
    let mut base = crate::models::base::ModelBase::default();
    base.rate = 3;
    let runs: Vec<bool> = (0..7).map(|_| !base.skip()).collect();
    assert_eq!(runs, [true, false, false, true, false, false, true]);
}

#[test]
fn unloaded_aerodynamics_refuse_to_run() {
    let mut fdm = Fdm::new().unwrap();
    assert!(matches!(fdm.run(), Err(Error::Model { .. })));
}
