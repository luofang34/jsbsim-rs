// SPDX-License-Identifier: LGPL-2.1-or-later

use std::collections::BTreeMap;

use super::super::daveml::Shot;
use super::{aero_ic, script, tolerance, without_control_law};

fn committed_f16() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/nasa/aircraft/f16/f16.xml");
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn the_control_law_gives_way_to_fixed_inputs_ahead_of_the_thrust() {
    let text = without_control_law(&committed_f16()).unwrap();
    assert!(!text.contains("<system name=\"F16_gnc\">"));
    assert!(!text.contains("<system name=\"air_now\">"));
    let inputs = text.find("<system name=\"shot inputs\">").unwrap();
    let prop = text.find("<system name=\"F16_prop\">").unwrap();
    assert!(inputs < prop);
    assert!(text.contains("<property value=\"0\">f16/control/pwr-pct</property>"));
    assert!(text.contains("<aerodynamics>") && text.contains("name=\"f16/aero/cxt\""));
}

#[test]
fn an_aero_shot_flies_level_at_its_airspeed_and_angles() {
    let shot = Shot {
        name: "s".to_owned(),
        inputs: BTreeMap::from(
            [
                ("trueAirspeed", 500.0),
                ("angleOfAttack", 30.0),
                ("angleOfSideslip", 0.0),
                ("bodyAngularRate_Roll", 0.5),
                ("bodyAngularRate_Pitch", 0.0),
                ("bodyAngularRate_Yaw", 0.0),
            ]
            .map(|(k, v)| (k.to_owned(), v)),
        ),
        ..Shot::default()
    };
    let ic = aero_ic("a0", &shot).unwrap();
    let u = 500.0 * 30.0_f64.to_radians().cos();
    let w = 500.0 * 30.0_f64.to_radians().sin();
    assert!(ic.contains(&format!("<x> {u:?} </x>")), "{ic}");
    assert!(ic.contains(&format!("<z> {w:?} </z>")), "{ic}");
    assert!(ic.contains("<pitch> 30.0 </pitch>") && ic.contains("<roll> 0.5 </roll>"));
    let missing = Shot::default();
    assert!(aero_ic("a1", &missing).is_err());
}

#[test]
fn scripts_override_the_fixed_inputs_and_tolerances_default_relative() {
    let s = script("p0", &[("f16/control/pwr-pct", 77.5)]);
    assert!(s.contains("<property value=\"77.5\"> f16/control/pwr-pct </property>"));
    assert!(s.contains("initialize=\"p0\""));
    assert_eq!(tolerance(1e-5, 3000.0), 1e-5);
    assert_eq!(tolerance(0.0, 3000.0), 3e-3);
    assert_eq!(tolerance(0.0, 0.1), 1e-6);
}
