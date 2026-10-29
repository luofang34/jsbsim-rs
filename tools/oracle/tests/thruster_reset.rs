// SPDX-License-Identifier: LGPL-2.1-or-later
//! JSBSim's `tests/TestThrusterReset.py` on the vendored aircraft: a reset
//! puts the thrusters back where they started (the F450's brushless
//! motors, whose engine has no `ResetToIC` of its own, stop their
//! propellers; the c310's constant-speed propeller gets its blade angle
//! back), and the run after the reset repeats the run before it exactly.
//! (The test's rotor case flies the ah1s, which the port does not load
//! yet.)

use std::path::Path;

use jsbsim::FileResolver;
use jsbsim::fdm::Fdm;

#[cfg(test)]
const STATE: [&str; 12] = [
    "position/lat-geod-rad",
    "position/long-gc-rad",
    "position/h-sl-ft",
    "attitude/phi-rad",
    "attitude/theta-rad",
    "attitude/psi-rad",
    "velocities/u-fps",
    "velocities/v-fps",
    "velocities/w-fps",
    "velocities/p-rad_sec",
    "velocities/q-rad_sec",
    "velocities/r-rad_sec",
];

#[cfg(test)]
fn outputs(fdm: &mut Fdm, names: &[&str]) -> Vec<f64> {
    names.iter().map(|p| fdm.get_property(p).unwrap()).collect()
}

/// `steps` frames, each after setting `inputs`, which a reset zeroes.
#[cfg(test)]
fn run_steps(fdm: &mut Fdm, inputs: &[(&str, f64)], names: &[&str], steps: usize) -> Vec<Vec<f64>> {
    (0..steps)
        .map(|_| {
            for &(name, value) in inputs {
                fdm.set_property(name, value).unwrap();
            }
            fdm.run().unwrap();
            outputs(fdm, names)
        })
        .collect()
}

/// `check_reset`: the thrusters leave their initial state, the reset
/// restores it, and the run repeats; the outputs at the reset.
#[cfg(test)]
fn check_reset(
    aircraft: &str,
    ic: &str,
    inputs: &[(&str, f64)],
    thruster: &[&str],
    steps: usize,
) -> Vec<f64> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/inputs");
    let resolver = FileResolver::new(&root);
    let mut fdm = Fdm::new().unwrap();
    fdm.load_model(&resolver, "aircraft", aircraft).unwrap();
    assert!(
        fdm.load_initial_conditions(&resolver, &format!("aircraft/{aircraft}"), ic)
            .unwrap()
    );
    fdm.run_ic().unwrap();
    let names: Vec<&str> = thruster.iter().chain(&STATE).copied().collect();
    let initial = outputs(&mut fdm, &names);
    let reference = run_steps(&mut fdm, inputs, &names, steps);
    for (name, value) in thruster.iter().zip(&initial) {
        assert_ne!(fdm.get_property(name).unwrap(), *value, "{name}");
    }
    fdm.reset_to_initial_conditions(0).unwrap();
    let at_reset = outputs(&mut fdm, &names);
    assert_eq!(at_reset, initial);
    let again = run_steps(&mut fdm, inputs, &names, steps);
    for (step, (state, reference)) in again.iter().zip(&reference).enumerate() {
        assert_eq!(state, reference, "step {step}");
    }
    at_reset
}

#[test]
fn a_reset_stops_the_brushless_motors_propellers_and_the_run_repeats() {
    let rpm = [
        "propulsion/engine[0]/propeller-rpm",
        "propulsion/engine[1]/propeller-rpm",
        "propulsion/engine[2]/propeller-rpm",
        "propulsion/engine[3]/propeller-rpm",
    ];
    let at_reset = check_reset(
        "F450",
        "initGrnd",
        &[("fcs/throttle-cmd-norm", 0.6)],
        &rpm,
        240,
    );
    assert_eq!(at_reset[..4], [0.0; 4]);
}

#[test]
fn a_reset_restores_the_blade_angle_and_the_run_repeats() {
    check_reset(
        "c310",
        "reset00",
        &[
            ("propulsion/magneto_cmd", 3.0),
            ("propulsion/starter_cmd", 1.0),
            ("fcs/mixture-cmd-norm[0]", 1.0),
            ("fcs/throttle-cmd-norm[0]", 1.0),
            ("fcs/advance-cmd-norm[0]", 0.5),
        ],
        &["propulsion/engine[0]/blade-angle"],
        600,
    );
}
