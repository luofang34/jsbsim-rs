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

#[test]
fn engines_in_an_included_propulsion_file_are_unsupported() {
    let ball = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/inputs/aircraft/ball/ball.xml"),
    )
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("aircraft/prop");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("prop.xml"),
        ball.replacen("<propulsion/>", "<propulsion file=\"Propulsion.xml\"/>", 1),
    )
    .unwrap();
    std::fs::write(
        dir.join("Propulsion.xml"),
        "<?xml version=\"1.0\"?>\n<propulsion>\n  <engine file=\"x\"/>\n</propulsion>\n",
    )
    .unwrap();
    let mut fdm = Fdm::new().unwrap();
    match fdm.load_model(&FileResolver::new(root.path()), "aircraft", "prop") {
        Err(jsbsim::Error::Unsupported { construct, .. }) => {
            assert!(construct.contains("<engine>"), "{construct}");
        }
        other => panic!("expected the included engine to be unsupported, got {other:?}"),
    }
}
