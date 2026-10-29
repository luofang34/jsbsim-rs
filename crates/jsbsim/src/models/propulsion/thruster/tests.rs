// SPDX-License-Identifier: LGPL-2.1-or-later
use crate::Error;
use crate::fdm::Fdm;
use crate::math::{E_X, E_Y, E_Z};
use crate::models::propulsion::tests::load;

#[cfg(test)]
fn direct(thruster: &str) -> String {
    format!(
        r#"<propulsion>
  <tank type="FUEL">
    <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
    <capacity>1000</capacity><contents>1000</contents>
  </tank>
  <engine>
    <feed>0</feed>
    <test_engine name="r0"><sl-fuel-flow>1</sl-fuel-flow></test_engine>
    <thruster>{thruster}</thruster>
  </engine>
</propulsion>"#
    )
}

#[cfg(test)]
fn running(fdm: &mut Fdm) {
    fdm.set_property("ic/h-sl-ft", 10000.0).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("propulsion/set-running", -1.0).unwrap();
    fdm.run().unwrap();
}

const ORIENTED: &str = r#"<direct name="d"/>
      <location unit="FT"><x>1</x><y>2</y><z>3</z></location>
      <orient unit="DEG"><roll>0</roll><pitch>90</pitch><yaw>0</yaw></orient>"#;

#[test]
fn the_location_and_orientation_load_with_their_properties() {
    let mut fdm = load(&direct(ORIENTED)).unwrap();
    for (axis, value) in [("x", 12.0), ("y", 24.0), ("z", 36.0)] {
        let reference = format!("propulsion/engine/{axis}-reference-position");
        assert_eq!(fdm.get_property(&reference).unwrap(), value);
        let acting = format!("propulsion/engine/{axis}-position");
        assert_eq!(fdm.get_property(&acting).unwrap(), value);
    }
    let pitch = fdm
        .get_property("propulsion/engine/pitch-angle-rad")
        .unwrap();
    assert_eq!(pitch, 90.0f64.to_radians());
    fdm.set_property("propulsion/engine/x-position", 6.0)
        .unwrap();
    let force = &fdm.models.propulsion.engines[0].thruster.base.force;
    assert_eq!(
        (force.location[E_X], force.acting_location[E_X]),
        (12.0, 6.0)
    );
}

#[test]
fn a_direct_thruster_pushes_along_its_orientation_scaled_by_the_reverser() {
    let mut fdm = load(&direct(ORIENTED)).unwrap();
    fdm.set_property("propulsion/engine/pitch-angle-rad", 0.0)
        .unwrap();
    running(&mut fdm);
    assert_eq!(
        fdm.get_property("propulsion/engine/thrust-lbs").unwrap(),
        1000.0
    );
    assert_eq!(fdm.get_property("forces/fbx-prop-lbs").unwrap(), 1000.0);
    let angle = 2.0f64.to_radians();
    fdm.set_property("propulsion/engine/reverser-angle-rad", angle)
        .unwrap();
    fdm.run().unwrap();
    let thrust = fdm.get_property("propulsion/engine/thrust-lbs").unwrap();
    assert_eq!(thrust, libm::cos(angle) * 1000.0);
    fdm.set_property(
        "propulsion/engine/yaw-angle-rad",
        0.5 * std::f64::consts::PI,
    )
    .unwrap();
    fdm.run().unwrap();
    let body = fdm.models.propulsion.forces;
    assert!(body[E_X].abs() < 1e-9 && (body[E_Y] - thrust).abs() < 1e-9);
    assert_eq!(body[E_Z], 0.0);
}

#[test]
fn a_fixed_pointing_has_no_angle_or_reverser_properties() {
    let thruster = r#"<direct name="d"/>
      <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
      <pointing><x>0</x><y>0</y><z>1</z></pointing>"#;
    let mut fdm = load(&direct(thruster)).unwrap();
    for path in [
        "propulsion/engine/pitch-angle-rad",
        "propulsion/engine/reverser-angle-rad",
    ] {
        assert!(fdm.node(path).unwrap().is_none(), "{path}");
    }
    running(&mut fdm);
    let body = fdm.models.propulsion.forces;
    assert_eq!((body[E_X], body[E_Y], body[E_Z]), (0.0, 0.0, 1000.0));
}

#[test]
fn only_a_direct_thruster_ties_a_reverser() {
    let thruster = r#"<test_thruster name="t"/>
      <location unit="IN"><x>0</x><y>0</y><z>0</z></location>"#;
    let fdm = load(&direct(thruster)).unwrap();
    assert!(
        fdm.node("propulsion/engine/pitch-angle-rad")
            .unwrap()
            .is_some()
    );
    let reverser = fdm.node("propulsion/engine/reverser-angle-rad");
    assert!(reverser.unwrap().is_none());
}

#[test]
fn an_unregistered_thruster_is_refused_before_it_ties_anything() {
    let err = load(&direct("<unregistered_thruster/>")).err().unwrap();
    assert!(
        matches!(&err, Error::Unsupported { construct, .. } if construct == "<unregistered_thruster>")
    );
}
