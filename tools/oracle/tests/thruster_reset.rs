// SPDX-License-Identifier: LGPL-2.1-or-later
//! JSBSim's `tests/TestThrusterReset.py` `test_constant_speed_propeller`
//! on the vendored c310: a reset puts the constant-speed propeller's blade
//! angle back where it started, and the run after the reset repeats the
//! run before it exactly. (The test's other cases fly a brushless DC motor
//! and a rotor, which the port does not have yet.)

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

/// The commands the test sets at each step: a reset zeroes them.
#[cfg(test)]
const INPUTS: [(&str, f64); 5] = [
    ("propulsion/magneto_cmd", 3.0),
    ("propulsion/starter_cmd", 1.0),
    ("fcs/mixture-cmd-norm[0]", 1.0),
    ("fcs/throttle-cmd-norm[0]", 1.0),
    ("fcs/advance-cmd-norm[0]", 0.5),
];

#[cfg(test)]
fn outputs(fdm: &mut Fdm, names: &[&str]) -> Vec<f64> {
    names.iter().map(|p| fdm.get_property(p).unwrap()).collect()
}

#[cfg(test)]
fn run_steps(fdm: &mut Fdm, names: &[&str], steps: usize) -> Vec<Vec<f64>> {
    (0..steps)
        .map(|_| {
            for (name, value) in INPUTS {
                fdm.set_property(name, value).unwrap();
            }
            fdm.run().unwrap();
            outputs(fdm, names)
        })
        .collect()
}

#[test]
fn a_reset_restores_the_blade_angle_and_the_run_repeats() {
    let inputs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/inputs");
    let resolver = FileResolver::new(&inputs);
    let mut fdm = Fdm::new().unwrap();
    fdm.load_model(&resolver, "aircraft", "c310").unwrap();
    assert!(
        fdm.load_initial_conditions(&resolver, "aircraft/c310", "reset00")
            .unwrap()
    );
    fdm.run_ic().unwrap();
    let thruster = ["propulsion/engine[0]/blade-angle"];
    let names: Vec<&str> = thruster.iter().chain(&STATE).copied().collect();
    let initial = outputs(&mut fdm, &names);
    let reference = run_steps(&mut fdm, &names, 600);
    assert_ne!(fdm.get_property(thruster[0]).unwrap(), initial[0]);
    fdm.reset_to_initial_conditions(0).unwrap();
    assert_eq!(outputs(&mut fdm, &names), initial);
    let again = run_steps(&mut fdm, &names, 600);
    for (step, (state, reference)) in again.iter().zip(&reference).enumerate() {
        assert_eq!(state, reference, "step {step}");
    }
}
