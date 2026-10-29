// SPDX-License-Identifier: LGPL-2.1-or-later
//! JSBSim's J246 tests on the vendored launch vehicle:
//! `tests/TestHoldDown.py` (held down on its pad, with or without its solid
//! rocket boosters lit, it does not move), `tests/TestFuelTanksInertia.py`
//! (after `RunIC` the fuel and oxidizer totals are the tanks' contents, and
//! a full cylindrical grain's inertia is that of a tube from its bore to
//! its radius) and `tests/TestPointMassInertia.py` (a point mass's tube
//! `<form>` flies as the inertia it stands for, written out).

use std::path::{Path, PathBuf};

use jsbsim::FileResolver;
use jsbsim::fdm::Fdm;
use jsbsim::xml::{Element, parse_document};

#[cfg(test)]
fn inputs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/inputs")
}

#[cfg(test)]
fn j246_text() -> String {
    std::fs::read_to_string(inputs().join("aircraft/J246/J246.xml")).unwrap()
}

#[cfg(test)]
fn propulsion(text: &str) -> Element {
    let root = parse_document(text.as_bytes(), "J246.xml").unwrap();
    root.find_element("propulsion").unwrap().clone()
}

/// `root` holding the vendored J246 with `J246.xml` replaced by `text`,
/// its engines and scripts/J2460.xml, as `CopyAircraftDef` and a rewrite
/// of the copy leave the sandbox.
#[cfg(test)]
fn sandbox(root: &Path, text: &str) {
    let src = inputs();
    let dir = root.join("aircraft/J246");
    std::fs::create_dir_all(dir.join("Systems")).unwrap();
    std::fs::write(dir.join("J246.xml"), text).unwrap();
    std::fs::copy(src.join("aircraft/J246/LC39.xml"), dir.join("LC39.xml")).unwrap();
    for entry in std::fs::read_dir(src.join("aircraft/J246/Systems")).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, dir.join("Systems").join(path.file_name().unwrap())).unwrap();
    }
    for sub in ["engine", "scripts"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    for engine in [
        "SRB",
        "SRB_nozzle",
        "SSME",
        "SSME_nozzle",
        "RL10",
        "RL10_nozzle",
    ] {
        let file = format!("engine/{engine}.xml");
        std::fs::copy(src.join(&file), root.join(&file)).unwrap();
    }
    let script = "scripts/J2460.xml";
    std::fs::copy(src.join(script), root.join(script)).unwrap();
}

/// scripts/J2460.xml loaded from `root` and initialised.
#[cfg(test)]
fn j2460(root: &Path) -> Fdm {
    let mut fdm = Fdm::new().unwrap();
    fdm.load_script(&FileResolver::new(root), "scripts/J2460.xml")
        .unwrap();
    fdm.run_ic().unwrap();
    fdm
}

#[cfg(test)]
fn get(fdm: &mut Fdm, name: &str) -> f64 {
    fdm.get_property(name).unwrap()
}

/// `assertAlmostEqual`: equal to 7 decimal places.
#[cfg(test)]
fn almost_equal(a: f64, b: f64) -> bool {
    ((a - b) * 1e7).round() == 0.0
}

/// `TestHoldDown`: for 420 s the vehicle's distance from the Earth's
/// centre stays within 1e-9 of its initial one.
#[cfg(test)]
fn stays_on_the_pad(boosters_lit: bool) {
    let root = inputs();
    let resolver = FileResolver::new(&root);
    let mut fdm = Fdm::new().unwrap();
    fdm.load_model(&resolver, "aircraft", "J246").unwrap();
    assert!(
        fdm.load_initial_conditions(&resolver, "aircraft/J246", "LC39")
            .unwrap()
    );
    fdm.set_property("forces/hold-down", 1.0).unwrap();
    fdm.run_ic().unwrap();
    let h0 = get(&mut fdm, "position/vrp-radius-ft");
    if boosters_lit {
        fdm.set_property("fcs/throttle-cmd-norm[0]", 1.0).unwrap();
        fdm.set_property("fcs/throttle-cmd-norm[1]", 1.0).unwrap();
    }
    let mut t = 0.0;
    let mut impulse: f64 = 0.0;
    while t < 420.0 {
        fdm.run().unwrap();
        t = get(&mut fdm, "simulation/sim-time-sec");
        let h = get(&mut fdm, "position/vrp-radius-ft");
        assert!((h / h0 - 1.0).abs() <= 1e-9, "t={t}: {h} vs {h0}");
        impulse = impulse.max(get(&mut fdm, "propulsion/engine[0]/total-impulse"));
    }
    assert_eq!(impulse > 0.0, boosters_lit, "{impulse}");
}

#[test]
fn held_down_it_stays_on_the_pad() {
    stays_on_the_pad(false);
}

#[test]
fn held_down_it_stays_on_the_pad_with_its_boosters_burning() {
    stays_on_the_pad(true);
}

#[test]
fn after_run_ic_the_totals_are_the_tanks_contents() {
    let (mut fuel, mut oxidizer) = (0.0, 0.0);
    for tank in propulsion(&j246_text()).find_elements("tank") {
        let contents = tank.find_element_value_as_number("contents").unwrap();
        match tank.attribute("type") {
            "FUEL" => fuel += contents,
            "OXIDIZER" => oxidizer += contents,
            other => panic!("tank type {other}"),
        }
    }
    let mut fdm = j2460(&inputs());
    assert!(almost_equal(
        get(&mut fdm, "propulsion/total-fuel-lbs"),
        fuel
    ));
    let total = get(&mut fdm, "propulsion/total-oxidizer-lbs");
    assert!(almost_equal(total, oxidizer));
}

/// `test_grain_tanks_content`, reading the inertia JSBSim ties: upstream
/// reads `propulsion/tank[0]local-ixx-slug_ft2`, which has no `/`.
#[test]
fn a_full_cylindrical_grain_is_a_tube_from_its_bore_to_its_radius() {
    let text = j246_text();
    let tank = propulsion(&text).find_element("tank").unwrap().clone();
    assert_eq!(
        tank.find_element("grain_config").unwrap().attribute("type"),
        "CYLINDRICAL"
    );
    let capacity = tank.find_element_value_as_number("capacity").unwrap();
    let contents = "<contents unit=\"LBS\"> 1114092 </contents>";
    assert!(text.contains(contents));
    for (fill, expected) in [(0.5 * capacity, 0.5 * capacity), (0.0, 0.0)] {
        let dir = tempfile::tempdir().unwrap();
        let replaced = format!("<contents unit=\"LBS\"> {fill} </contents>");
        sandbox(dir.path(), &text.replacen(contents, &replaced, 1));
        let mut fdm = j2460(dir.path());
        let first = "propulsion/tank[0]/contents-lbs";
        assert!(almost_equal(get(&mut fdm, first), expected));
        fdm.set_property("propulsion/tank/contents-lbs", capacity)
            .unwrap();
        // Radius 6 ft, bore 2 ft.
        let mass = capacity / 32.174049;
        let ixx = 0.5 * mass * (6.0 * 6.0 + 1.0 * 1.0);
        let local = get(&mut fdm, "propulsion/tank[0]/local-ixx-slug_ft2");
        assert!((local - ixx).abs() < 1e-9 * ixx, "{local} vs {ixx}");
    }
}

/// The properties `TestPointMassInertia` compares through tests/output.xml
/// that the mass properties reach.
#[cfg(test)]
const COMPARED: [&str; 14] = [
    "inertia/ixx-slugs_ft2",
    "inertia/iyy-slugs_ft2",
    "inertia/izz-slugs_ft2",
    "inertia/ixz-slugs_ft2",
    "inertia/cg-x-in",
    "inertia/weight-lbs",
    "position/vrp-radius-ft",
    "attitude/phi-rad",
    "attitude/theta-rad",
    "attitude/psi-rad",
    "velocities/p-rad_sec",
    "velocities/q-rad_sec",
    "velocities/r-rad_sec",
    "accelerations/qdot-rad_sec2",
];

#[test]
fn a_tube_form_is_the_inertia_it_stands_for() {
    let text = j246_text();
    let form = "<form shape=\"tube\">\n          <radius unit=\"FT\"> 6 </radius>\n          <length unit=\"FT\"> 145 </length>\n        </form>";
    assert!(text.contains(form));
    let weight = 185000.0;
    let (radius, length): (f64, f64) = (6.0, 145.0);
    let ixx = radius * radius;
    let iyy = (6.0 * ixx + length * length) / 12.0;
    let slugs = |i: f64| i * weight / 32.174049;
    let explicit = format!(
        "<ixx>{}</ixx><iyy>{}</iyy><izz>{}</izz>",
        slugs(ixx),
        slugs(iyy),
        slugs(iyy)
    );
    let dir = tempfile::tempdir().unwrap();
    sandbox(dir.path(), &text.replacen(form, &explicit, 1));
    let mut reference = j2460(&inputs());
    let mut modified = j2460(dir.path());
    while get(&mut reference, "simulation/sim-time-sec") < 50.0 {
        assert!(reference.run().unwrap());
        assert!(modified.run().unwrap());
        for name in COMPARED {
            let (a, b) = (get(&mut reference, name), get(&mut modified, name));
            assert!((a - b).abs() <= 1e-8, "{name}: {a} vs {b}");
        }
    }
}
