// SPDX-License-Identifier: LGPL-2.1-or-later
//! The vendored ball loads through the whole-aircraft loader, and so do the
//! engine and thruster files an included propulsion file names.

use std::path::Path;

use jsbsim::Fdm;
use jsbsim::FileResolver;

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
fn engines_in_an_included_propulsion_file_load_from_their_files() {
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
        "<?xml version=\"1.0\"?>\n<propulsion>\n  <engine file=\"x\">\n    <thruster file=\"p\"/>\n  </engine>\n</propulsion>\n",
    )
    .unwrap();
    let engines = root.path().join("engine");
    std::fs::create_dir_all(&engines).unwrap();
    std::fs::write(
        engines.join("x.xml"),
        "<?xml version=\"1.0\"?>\n<rocket_engine name=\"x\"><isp>300</isp></rocket_engine>\n",
    )
    .unwrap();
    std::fs::write(
        engines.join("p.xml"),
        "<?xml version=\"1.0\"?>\n<nozzle name=\"p\"><area>2</area></nozzle>\n",
    )
    .unwrap();
    let mut fdm = Fdm::new().unwrap();
    fdm.load_model(&FileResolver::new(root.path()), "aircraft", "prop")
        .unwrap();
    assert_eq!(fdm.get_property("propulsion/engine[0]/isp").unwrap(), 300.0);
}
