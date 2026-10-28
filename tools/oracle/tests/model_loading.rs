// SPDX-License-Identifier: LGPL-2.1-or-later
//! The vendored ball loads through the whole-aircraft loader.

use std::path::Path;

use jsbsim::FileResolver;
use jsbsim::fdm::Fdm;

#[test]
fn the_vendored_ball_loads() {
    let inputs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/inputs");
    let mut fdm = Fdm::new().unwrap();
    fdm.load_model(&FileResolver::new(&inputs), "aircraft", "ball")
        .unwrap();
    assert!(fdm.get_property("inertia/weight-lbs").unwrap() > 0.0);
}

#[test]
fn contact_values_cpp_leaves_uninitialized_start_at_zero() {
    let inputs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/inputs");
    let mut fdm = Fdm::new().unwrap();
    fdm.load_model(&FileResolver::new(&inputs), "aircraft", "ball")
        .unwrap();
    for path in ["gear/unit/AGL-ft", "gear/unit/side_friction_coeff"] {
        assert_eq!(fdm.get_property(path).unwrap(), 0.0, "{path}");
    }
}
