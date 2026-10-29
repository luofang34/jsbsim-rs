// SPDX-License-Identifier: LGPL-2.1-or-later
use super::{ControlMap, Rotor, RpmDrive};
use crate::fdm::Fdm;
use crate::models::propulsion::families::ThrusterKind;
use crate::models::propulsion::tests::load;

/// A main rotor driven by an electric engine, and a tail rotor whose speed
/// engine 0's rotor dictates through its gear.
const HELICOPTER: &str = r#"<propulsion>
  <engine>
    <electric_engine name="e"><power unit="HP">1500</power></electric_engine>
    <thruster>
      <location unit="IN"><x>176</x><y>0</y><z>153</z></location>
      <orient unit="DEG"><roll>0</roll><pitch>90</pitch><yaw>0</yaw></orient>
      <sense>1</sense>
      <rotor name="main">
        <diameter unit="FT">44</diameter><numblades>2</numblades>
        <gearratio>20.37</gearratio><nominalrpm>324</nominalrpm>
        <chord unit="FT">2.25</chord><flappingmoment unit="SLUG*FT2">1382</flappingmoment>
        <twist>-0.175</twist><massmoment>85</massmoment>
        <polarmoment unit="SLUG*FT2">2900</polarmoment><inflowlag>0.09</inflowlag>
        <hingeoffset unit="FT">3.3</hingeoffset>
        <groundeffectexp>0.19</groundeffectexp>
      </rotor>
    </thruster>
  </engine>
  <engine>
    <electric_engine name="d"/>
    <thruster>
      <location unit="IN"><x>496</x><y>16</y><z>119</z></location>
      <orient unit="DEG"><roll>0</roll><pitch>0</pitch><yaw>90</yaw></orient>
      <rotor name="tail">
        <diameter unit="FT">8.5</diameter><numblades>2</numblades>
        <gearratio>3.9759</gearratio><nominalrpm>1660</nominalrpm>
        <ExternalRPM>0</ExternalRPM><controlmap>tail</controlmap>
      </rotor>
    </thruster>
  </engine>
</propulsion>"#;

#[cfg(test)]
fn rotor(fdm: &Fdm, n: usize) -> &Rotor {
    match &fdm.models.propulsion.engines[n].thruster.kind {
        ThrusterKind::Rotor(r) => r,
        _ => panic!("not a rotor"),
    }
}

#[test]
fn configure_estimates_what_the_rotor_leaves_out() {
    let fdm = load(HELICOPTER).unwrap();
    let tail = rotor(&fdm, 1);
    assert_eq!(tail.controls.map, ControlMap::Tail);
    assert_eq!(tail.config.radius, 4.25);
    assert_eq!(tail.config.lift_curve_slope, 6.0);
    assert_eq!(tail.config.blade_twist, -0.17);
    assert_eq!(tail.config.hinge_offset, 0.05 * 4.25);
    // The solidity guess, constrained to 0.14, times pi R / N.
    let chord = 0.14 * std::f64::consts::PI * 4.25 / 2.0;
    assert_eq!(tail.config.blade_chord, chord);
    let RpmDrive::External(ext) = &tail.drive else {
        panic!("the tail rotor is driven by the main rotor");
    };
    assert_eq!(ext.definition, 0);
    assert_eq!(ext.source_gear_ratio, 20.37);
    assert!(ext.source.is_some());
    let main = rotor(&fdm, 0);
    assert_eq!(
        fdm.models.propulsion.engines[0].thruster.base.gear_ratio,
        20.37
    );
    let RpmDrive::Transmission(t) = &main.drive else {
        panic!("the main rotor has a transmission");
    };
    assert_eq!(t.thruster_moment, 2900.0);
    assert_eq!(t.engine_moment, 290.0);
}

#[test]
fn the_controls_tied_follow_the_control_map() {
    let fdm = load(HELICOPTER).unwrap();
    let has = |p: &str| fdm.tree().has_node(p).unwrap();
    assert!(has("propulsion/engine[0]/collective-ctrl-rad"));
    assert!(has("propulsion/engine[0]/lateral-ctrl-rad"));
    assert!(has("propulsion/engine[0]/clutch-ctrl-norm"));
    assert!(has("propulsion/engine[1]/antitorque-ctrl-rad"));
    assert!(!has("propulsion/engine[1]/lateral-ctrl-rad"));
    assert!(!has("propulsion/engine[1]/clutch-ctrl-norm"));
    assert!(!has("propulsion/engine[1]/x-rpm-dict"));
}

#[test]
fn the_engine_spins_the_rotor_up_and_the_tail_rotor_follows_through_the_gears() {
    let mut fdm = load(HELICOPTER).unwrap();
    fdm.set_property("ic/h-sl-ft", 1000.0).unwrap();
    // The light test airframe would tumble.
    fdm.set_property("forces/hold-down", 1.0).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("fcs/throttle-cmd-norm[0]", 1.0).unwrap();
    fdm.set_property("propulsion/engine[0]/collective-ctrl-rad", 0.2)
        .unwrap();
    for _ in 0..120 {
        fdm.run().unwrap();
    }
    let main_rpm = fdm.get_property("propulsion/engine[0]/rotor-rpm").unwrap();
    assert!(main_rpm > 1.0, "{main_rpm}");
    let engine_rpm = fdm.get_property("propulsion/engine[0]/engine-rpm").unwrap();
    let t = match &rotor(&fdm, 0).drive {
        RpmDrive::Transmission(t) => t.engine_rpm,
        RpmDrive::External(_) => panic!("no transmission"),
    };
    assert_eq!(engine_rpm, t * 20.37);
    // The tail rotor read the main rotor's speed of the frame before.
    fdm.run().unwrap();
    let tail = rotor(&fdm, 1);
    assert!(tail.state.rpm > 1.0);
    assert!(fdm.get_property("propulsion/engine[0]/thrust-lbs").unwrap() > 0.0);
}

#[test]
fn a_reset_stops_the_rotor() {
    let mut fdm = load(HELICOPTER).unwrap();
    fdm.set_property("forces/hold-down", 1.0).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("fcs/throttle-cmd-norm[0]", 1.0).unwrap();
    for _ in 0..60 {
        fdm.run().unwrap();
    }
    fdm.reset_to_initial_conditions(0).unwrap();
    let main = rotor(&fdm, 0);
    // RunIC's frame holds it at its minimal speed.
    assert_eq!(main.state.rpm, 1.0);
    let RpmDrive::Transmission(t) = &main.drive else {
        panic!("no transmission");
    };
    assert_eq!((t.engine_rpm, t.thruster_rpm), (0.0, 0.0));
}
