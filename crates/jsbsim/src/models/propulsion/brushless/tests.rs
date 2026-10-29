// SPDX-License-Identifier: LGPL-2.1-or-later
use super::{BrushLessDcMotor, WATT_PER_RPM_TO_FTPOUND};
use crate::Error;
use crate::fdm::Fdm;
use crate::models::propulsion::families::EngineKind;
use crate::models::propulsion::tests::load;

const MOTOR: &str = "<velocityconstant>960</velocityconstant>
      <coilresistance>0.117</coilresistance>
      <noloadcurrent>0.45</noloadcurrent>
      <maxvolts>14.63</maxvolts>";

#[cfg(test)]
fn engine(motor: &str) -> String {
    format!(
        r#"<propulsion>
  <engine>
    <brushless_dc_motor name="m">{motor}</brushless_dc_motor>
    <thruster>
      <location unit="IN"><x>0</x><y>0</y><z>-5</z></location>
      <propeller name="p">
        <ixx>0.0001</ixx><diameter unit="IN">9.4</diameter><numblades>2</numblades>
        <table name="C_THRUST" type="internal">
          <tableData>
            0.0 0.11
            1.0 0.0
          </tableData>
        </table>
        <table name="C_POWER" type="internal">
          <tableData>
            0.0 0.04
            1.0 0.02
          </tableData>
        </table>
      </propeller>
    </thruster>
  </engine>
</propulsion>"#
    )
}

#[cfg(test)]
fn motor(fdm: &Fdm) -> &BrushLessDcMotor {
    match &fdm.models.propulsion.engines[0].kind {
        EngineKind::BrushLessDcMotor(m) => m,
        _ => panic!("not a brushless motor"),
    }
}

#[test]
fn every_constant_is_mandatory() {
    let m = motor_of(&load(&engine(MOTOR)).unwrap());
    assert_eq!(m.power_watts, (14.63 / 0.117 + 0.45) * 14.63);
    for name in [
        "velocityconstant",
        "coilresistance",
        "noloadcurrent",
        "maxvolts",
    ] {
        let start = MOTOR.find(&format!("<{name}>")).unwrap();
        let end = MOTOR.find(&format!("</{name}>")).unwrap() + name.len() + 3;
        let text = engine(&MOTOR.replace(&MOTOR[start..end], ""));
        assert!(matches!(load(&text), Err(Error::Model { .. })), "{name}");
    }
}

#[cfg(test)]
fn motor_of(fdm: &Fdm) -> BrushLessDcMotor {
    motor(fdm).clone()
}

#[test]
fn the_throttle_drives_a_current_that_turns_the_propeller() {
    let mut fdm = load(&engine(MOTOR)).unwrap();
    fdm.run_ic().unwrap();
    // At rest the friction cancels up to the no-load current.
    fdm.run().unwrap();
    assert_eq!(motor(&fdm).current, 0.0);
    fdm.set_property("fcs/throttle-cmd-norm", 0.6).unwrap();
    for _ in 0..240 {
        fdm.run().unwrap();
    }
    let m = motor_of(&fdm);
    let rpm = fdm
        .get_property("propulsion/engine[0]/propeller-rpm")
        .unwrap();
    assert!(rpm > 1000.0, "{rpm}");
    let volts = 14.63 * 0.6;
    assert!(
        m.current > 0.45 && m.current < volts / 0.117,
        "{}",
        m.current
    );
    assert_eq!(
        fdm.get_property("propulsion/engine[0]/current-amperes")
            .unwrap(),
        m.current
    );
    assert!(fdm.get_property("propulsion/engine[0]/power-hp").unwrap() > 0.0);
    assert_eq!(
        WATT_PER_RPM_TO_FTPOUND,
        60.0 / (2.0 * std::f64::consts::PI * 1.3558)
    );
}
