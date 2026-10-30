// SPDX-License-Identifier: LGPL-2.1-or-later
//! JSBSim's `tests/TestBrushlessDCMotor.py` on the vendored F450: cutting
//! the throttle stops the propellers, and the friction resists the
//! rotation with the torque the current would give in the other sense.

use std::path::Path;

use jsbsim::Fdm;
use jsbsim::FileResolver;

/// Below the F450's hover throttle: it stays on the ground.
#[cfg(test)]
const SPIN_UP_THROTTLE: f64 = 0.3;
/// 4 s at 120 Hz.
#[cfg(test)]
const SPIN_UP_STEPS: usize = 480;
/// 1 s at 120 Hz.
#[cfg(test)]
const STOP_STEPS: usize = 120;

/// `engine/DJI_E305.xml`'s constants.
#[cfg(test)]
const MAX_VOLTS: f64 = 14.63;
#[cfg(test)]
const KV: f64 = 960.0;
#[cfg(test)]
const RESISTANCE: f64 = 0.117;
#[cfg(test)]
const I0: f64 = 0.45;

#[cfg(test)]
fn f450() -> Fdm {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/inputs");
    let resolver = FileResolver::new(&root);
    let mut fdm = Fdm::new().unwrap();
    fdm.load_model(&resolver, "aircraft", "F450").unwrap();
    assert!(
        fdm.load_initial_conditions(&resolver, "aircraft/F450", "initGrnd")
            .unwrap()
    );
    fdm.run_ic().unwrap();
    fdm
}

#[cfg(test)]
fn run_steps(fdm: &mut Fdm, throttle: f64, steps: usize) {
    fdm.set_property("fcs/throttle-cmd-norm", throttle).unwrap();
    for _ in 0..steps {
        fdm.run().unwrap();
    }
}

#[cfg(test)]
fn get(fdm: &mut Fdm, name: &str) -> f64 {
    fdm.get_property(name).unwrap()
}

/// The motor torque, up to a constant factor, over the current left once
/// the friction is overcome: (5) of Drela's "First-Order DC Electric Motor
/// Model".
#[cfg(test)]
fn torque_per_friction_free_current(fdm: &mut Fdm, n: usize) -> f64 {
    let current = get(fdm, &format!("propulsion/engine[{n}]/current-amperes"));
    let volts = MAX_VOLTS * get(fdm, &format!("fcs/throttle-pos-norm[{n}]"));
    let rpm = (volts - current * RESISTANCE) * KV;
    get(fdm, &format!("propulsion/engine[{n}]/power-hp")) / rpm / (current - I0)
}

#[test]
fn cutting_the_throttle_stops_the_propellers() {
    let mut fdm = f450();
    run_steps(&mut fdm, SPIN_UP_THROTTLE, SPIN_UP_STEPS);
    for n in 0..4 {
        let rpm = get(&mut fdm, &format!("propulsion/engine[{n}]/propeller-rpm"));
        assert!(rpm > 1000.0, "engine {n}: {rpm}");
    }
    run_steps(&mut fdm, 0.0, STOP_STEPS);
    for n in 0..4 {
        assert_eq!(get(&mut fdm, &format!("fcs/throttle-pos-norm[{n}]")), 0.0);
        let rpm = get(&mut fdm, &format!("propulsion/engine[{n}]/propeller-rpm"));
        assert_eq!(rpm, 0.0, "engine {n}");
    }
}

#[test]
fn the_friction_resists_the_rotation() {
    let mut fdm = f450();
    run_steps(&mut fdm, SPIN_UP_THROTTLE, SPIN_UP_STEPS);
    let driving: Vec<f64> = (0..4)
        .map(|n| torque_per_friction_free_current(&mut fdm, n))
        .collect();
    // Effectors.xml runs before FlightControl.xml: the cut needs 2 frames.
    run_steps(&mut fdm, 0.0, 2);
    for (n, driving) in driving.iter().enumerate() {
        let current = get(&mut fdm, &format!("propulsion/engine[{n}]/current-amperes"));
        assert!(current < -I0, "engine {n}: {current}");
        let braking = torque_per_friction_free_current(&mut fdm, n);
        assert!((braking / driving - 1.0).abs() < 5e-10, "engine {n}");
    }
}
