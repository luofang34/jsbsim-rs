// SPDX-License-Identifier: LGPL-2.1-or-later
use super::Propeller;
use crate::Error;
use crate::fdm::Fdm;
use crate::models::propulsion::families::ThrusterKind;
use crate::models::propulsion::tests::load;

const FIXED: &str = r#"<propeller name="p">
        <ixx>2</ixx><diameter unit="IN">72</diameter><numblades>2</numblades>
        <gearratio>1.5</gearratio>
        <table name="C_THRUST" type="internal">
          <tableData>
            0.0 0.1
            1.0 0.0
          </tableData>
        </table>
        <table name="C_POWER" type="internal">
          <tableData>
            0.0 0.05
            1.0 0.02
          </tableData>
        </table>
      </propeller>
      <sense>-1</sense>"#;

const GOVERNED: &str = r#"<propeller name="p">
        <ixx>2</ixx><diameter unit="FT">6</diameter>
        <minpitch>10</minpitch><maxpitch>40</maxpitch>
        <minrpm>1000</minrpm><maxrpm>2000</maxrpm>
        <table name="C_THRUST" type="internal">
          <tableData>
                  10    40
            0.0   0.1   0.2
            1.0   0.0   0.1
          </tableData>
        </table>
        <table name="C_POWER" type="internal">
          <tableData>
                  10    40
            0.0   0.04  0.09
            1.0   0.02  0.06
          </tableData>
        </table>
      </propeller>"#;

#[cfg(test)]
fn engine(thruster: &str) -> String {
    format!(
        r#"<propulsion>
  <tank type="FUEL">
    <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
    <capacity>1000</capacity><contents>1000</contents>
  </tank>
  <engine>
    <feed>0</feed>
    <test_engine name="r0"><sl-fuel-flow>0.01</sl-fuel-flow></test_engine>
    <thruster>
      <location unit="IN"><x>-40</x><y>0</y><z>0</z></location>
      {thruster}
    </thruster>
  </engine>
</propulsion>"#
    )
}

#[cfg(test)]
fn propeller(fdm: &mut Fdm) -> &mut Propeller {
    match &mut fdm.models.propulsion.engines[0].thruster.kind {
        ThrusterKind::Propeller(p) => p,
        _ => panic!("not a propeller"),
    }
}

#[cfg(test)]
fn running(fdm: &mut Fdm) {
    fdm.set_property("ic/h-sl-ft", 5000.0).unwrap();
    fdm.set_property("ic/u-fps", 5.0).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("propulsion/set-running", -1.0).unwrap();
}

#[test]
fn a_fixed_pitch_propeller_loads_its_geometry_and_properties() {
    let mut fdm = load(&engine(FIXED)).unwrap();
    let p = propeller(&mut fdm);
    assert_eq!((p.diameter, p.num_blades, p.sense), (6.0, 2, -1.0));
    assert_eq!((p.d4, p.d5), (1296.0, 7776.0));
    assert!(!p.is_vpitch());
    assert_eq!(
        fdm.models.propulsion.engines[0].thruster.base.gear_ratio,
        1.5
    );
    for path in [
        "engine-rpm",
        "advance-ratio",
        "blade-angle",
        "thrust-coefficient",
        "propeller-rpm",
        "helical-tip-Mach",
        "constant-speed-mode",
        "prop-induced-velocity_fps",
        "propeller-power-ftlbps",
        "propeller-torque-ftlb",
        "propeller-sense",
    ] {
        let path = format!("propulsion/engine/{path}");
        assert!(fdm.node(&path).unwrap().is_some(), "{path}");
    }
    assert_eq!(
        fdm.get_property("propulsion/engine/propeller-sense")
            .unwrap(),
        -1.0
    );
}

#[test]
fn the_power_tables_are_required_and_the_p_factor_positive() {
    let no_power = FIXED.replace("C_POWER", "C_POWERX");
    assert!(matches!(load(&engine(&no_power)), Err(Error::Model { .. })));
    let negative = format!("{FIXED}<p_factor>-1</p_factor>");
    assert!(matches!(load(&engine(&negative)), Err(Error::Model { .. })));
}

#[test]
fn the_engine_power_spins_the_propeller_up_and_its_thrust_follows() {
    let mut fdm = load(&engine(FIXED)).unwrap();
    propeller(&mut fdm).rpm = 100.0;
    fdm.set_property("ic/h-sl-ft", 5000.0).unwrap();
    fdm.set_property("ic/u-fps", 5.0).unwrap();
    fdm.run_ic().unwrap();
    fdm.models.propulsion.engines[0].base.running = true;
    let mut last = 0.0;
    for _ in 0..50 {
        fdm.run().unwrap();
        let rpm = fdm.get_property("propulsion/engine/propeller-rpm").unwrap();
        assert!(rpm > last);
        last = rpm;
    }
    // Starting it reaches the speed where the propeller absorbs the power.
    fdm.set_property("propulsion/set-running", -1.0).unwrap();
    let absorbed = fdm
        .get_property("propulsion/engine/propeller-power-ftlbps")
        .unwrap();
    assert!((absorbed - 1000.0).abs() < 0.1, "{absorbed}");
    let last = fdm.get_property("propulsion/engine/propeller-rpm").unwrap();
    let engine_rpm = fdm.get_property("propulsion/engine/engine-rpm").unwrap();
    assert_eq!(engine_rpm, last * 1.5);
    let p = propeller(&mut fdm);
    assert!(p.j > 0.0 && p.helical_tip_mach > 0.0 && p.vinduced > 0.0);
    assert!(
        p.torque[crate::math::E_X] > 0.0,
        "sense -1 turns the torque around"
    );
    let thrust = fdm.get_property("propulsion/engine/thrust-lbs").unwrap();
    assert!(thrust > 0.0);
}

#[test]
fn the_governor_moves_the_blades_toward_the_commanded_rpm() {
    let mut fdm = load(&engine(GOVERNED)).unwrap();
    assert_eq!(
        fdm.get_property("propulsion/engine/constant-speed-mode")
            .unwrap(),
        1.0
    );
    assert_eq!(
        fdm.get_property("propulsion/engine/blade-angle").unwrap(),
        10.0
    );
    running(&mut fdm);
    let p = propeller(&mut fdm);
    p.rpm = 2500.0;
    p.advance = 0.5;
    fdm.run().unwrap();
    let pitch = fdm.get_property("propulsion/engine/blade-angle").unwrap();
    assert_eq!(
        pitch,
        10.0 + 1000.0 * (1.0 / 120.0),
        "1000 rpm over the 1500 commanded"
    );
    let p = propeller(&mut fdm);
    p.rpm = 150.0;
    p.pitch = 20.0;
    fdm.run().unwrap();
    let pitch = fdm.get_property("propulsion/engine/blade-angle").unwrap();
    assert_eq!(pitch, 20.0, "below 200 rpm the blades do not move");
}

#[test]
fn feathering_drives_the_blades_to_max_pitch() {
    let mut fdm = load(&engine(GOVERNED)).unwrap();
    running(&mut fdm);
    propeller(&mut fdm).feathered = true;
    for _ in 0..3 {
        fdm.run().unwrap();
    }
    let pitch = propeller(&mut fdm).pitch;
    let mut expected = 10.0;
    for _ in 0..3 {
        expected += (40.0 - expected) / 300.0;
    }
    assert_eq!(pitch, expected);
}

#[test]
fn reset_stops_the_propeller_and_restores_the_blade_angle() {
    let mut fdm = load(&engine(GOVERNED)).unwrap();
    running(&mut fdm);
    for _ in 0..20 {
        fdm.run().unwrap();
    }
    let p = propeller(&mut fdm);
    p.pitch = 30.0;
    assert!(p.rpm > 0.0);
    fdm.models.propulsion.init_model();
    let p = propeller(&mut fdm);
    assert_eq!((p.rpm, p.pitch, p.vinduced), (0.0, 10.0, 0.0));
}
