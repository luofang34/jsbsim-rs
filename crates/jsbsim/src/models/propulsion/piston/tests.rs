// SPDX-License-Identifier: LGPL-2.1-or-later
use super::Piston;
use super::cycle::std_pressure_100k;
use crate::fdm::Fdm;
use crate::models::propulsion::families::EngineKind;
use crate::models::propulsion::tests::load;

const BOOST: &str = "<numboostspeeds>2</numboostspeeds>
      <ratedboost1 unit=\"PSI\">10</ratedboost1><ratedboost2 unit=\"PSI\">9</ratedboost2>
      <ratedpower1 unit=\"HP\">1200</ratedpower1><ratedpower2 unit=\"HP\">1100</ratedpower2>
      <ratedrpm1>3000</ratedrpm1><ratedrpm2>3000</ratedrpm2>
      <ratedaltitude1 unit=\"FT\">8000</ratedaltitude1>
      <ratedaltitude2 unit=\"FT\">20000</ratedaltitude2>";

#[cfg(test)]
fn engine(extra: &str) -> String {
    format!(
        r#"<propulsion>
  <tank type="FUEL">
    <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
    <capacity>200</capacity><contents>200</contents>
  </tank>
  <engine>
    <feed>0</feed>
    <piston_engine name="p">
      <minmp unit="INHG">10.0</minmp><maxmp unit="INHG">28.5</maxmp>
      <displacement unit="IN3">320.0</displacement>
      <maxhp>160.0</maxhp><bsfc>0.32</bsfc><cycles>4.0</cycles>
      <idlerpm>550.0</idlerpm><maxrpm>2700.0</maxrpm>
      <minthrottle>0.1</minthrottle><sparkfaildrop>0.1</sparkfaildrop>
      {extra}
    </piston_engine>
    <thruster>
      <location unit="IN"><x>-40</x><y>0</y><z>0</z></location>
      <propeller name="p">
        <ixx>1.67</ixx><diameter unit="IN">75</diameter><numblades>2</numblades>
        <table name="C_THRUST" type="internal">
          <tableData>
            0.0 0.073
            1.0 0.024
            2.0 -0.069
          </tableData>
        </table>
        <table name="C_POWER" type="internal">
          <tableData>
            0.0 0.066
            1.0 0.028
            2.0 0.052
          </tableData>
        </table>
      </propeller>
    </thruster>
  </engine>
</propulsion>"#
    )
}

#[cfg(test)]
fn piston(fdm: &Fdm) -> &Piston {
    match &fdm.models.propulsion.engines[0].kind {
        EngineKind::Piston(p) => p,
        _ => panic!("not a piston engine"),
    }
}

#[cfg(test)]
fn on_ground() -> Fdm {
    let mut fdm = load(&engine("")).unwrap();
    fdm.set_property("ic/h-sl-ft", 0.0).unwrap();
    fdm.run_ic().unwrap();
    fdm
}

#[cfg(test)]
fn run(fdm: &mut Fdm, frames: u32) {
    for _ in 0..frames {
        fdm.run().unwrap();
    }
}

#[test]
fn the_constructor_derives_its_limits_and_ties_its_properties() {
    let fdm = load(&engine("")).unwrap();
    let p = piston(&fdm);
    assert_eq!(p.config.spark_fail_drop, 0.9);
    assert_eq!(p.config.starter_rpm, 1100.0, "twice the idle speed");
    assert_eq!(p.config.starter_torque, 64.0, "0.4 times the power");
    assert_eq!(p.config.oil_press_rpm_max, 2700.0 * 0.75);
    assert!(p.tuning.z_airbox > 0.0 && p.config.z_throttle > 0.0);
    let base = &fdm.models.propulsion.engines[0].base;
    assert_eq!((base.min_throttle, base.max_throttle), (0.1, 1.0));
    for path in [
        "power-hp",
        "bsfc-lbs_hphr",
        "map-inhg",
        "egt-degF",
        "cht-degF",
        "oil-pressure-psi",
        "AFR",
        "boost-speed",
    ] {
        let path = format!("propulsion/engine/{path}");
        assert!(fdm.node(&path).unwrap().is_some(), "{path}");
    }
    assert!(
        fdm.node("propulsion/engine/boostloss-hp")
            .unwrap()
            .is_none()
    );
    assert!(fdm.node("propulsion/magneto_cmd").unwrap().is_some());
}

#[test]
fn the_starter_and_magnetos_start_the_engine_and_cutting_them_stops_it() {
    let mut fdm = on_ground();
    fdm.set_property("fcs/throttle-cmd-norm", 0.2).unwrap();
    fdm.set_property("fcs/mixture-cmd-norm", 1.0).unwrap();
    fdm.set_property("propulsion/starter_cmd", 1.0).unwrap();
    run(&mut fdm, 120);
    assert!(fdm.get_property("propulsion/engine/engine-rpm").unwrap() > 0.0);
    assert_eq!(
        fdm.get_property("propulsion/engine/set-running").unwrap(),
        0.0
    );
    fdm.set_property("propulsion/magneto_cmd", 3.0).unwrap();
    run(&mut fdm, 600);
    assert_eq!(
        fdm.get_property("propulsion/engine/set-running").unwrap(),
        1.0
    );
    fdm.set_property("propulsion/starter_cmd", 0.0).unwrap();
    run(&mut fdm, 240);
    assert_eq!(
        fdm.get_property("propulsion/engine/set-running").unwrap(),
        1.0
    );
    let both = fdm.get_property("propulsion/engine/power-hp").unwrap();
    fdm.set_property("propulsion/magneto_cmd", 1.0).unwrap();
    run(&mut fdm, 1);
    let left = fdm.get_property("propulsion/engine/power-hp").unwrap();
    assert!(left < both, "one magneto drops the power");
    assert!(piston(&fdm).state.magneto_left && !piston(&fdm).state.magneto_right);
    fdm.set_property("propulsion/magneto_cmd", 0.0).unwrap();
    run(&mut fdm, 1);
    assert_eq!(
        fdm.get_property("propulsion/engine/set-running").unwrap(),
        0.0
    );
}

#[test]
fn starting_it_running_sets_both_magnetos_and_twice_the_idle_speed() {
    let mut fdm = load(&engine("")).unwrap();
    fdm.set_property("ic/h-sl-ft", 3000.0).unwrap();
    fdm.set_property("ic/u-fps", 150.0).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("propulsion/set-running", 0.0).unwrap();
    assert_eq!(piston(&fdm).state.magnetos, 3);
    assert_eq!(
        fdm.get_property("propulsion/engine/set-running").unwrap(),
        1.0
    );
    run(&mut fdm, 10);
    assert!(fdm.get_property("propulsion/engine/thrust-lbs").unwrap() > 0.0);
    let afr = fdm.get_property("propulsion/engine/AFR").unwrap();
    assert!(afr.is_finite() && afr > 10.0);
    // A cut-off mixture starves the cylinders.
    fdm.set_property("fcs/mixture-cmd-norm", 0.0).unwrap();
    run(&mut fdm, 2);
    assert_eq!(
        fdm.get_property("propulsion/engine/fuel-flow-rate-pps")
            .unwrap(),
        0.0
    );
    assert_eq!(
        fdm.get_property("propulsion/engine/set-running").unwrap(),
        0.0
    );
    assert_eq!(
        fdm.get_property("propulsion/engine/AFR").unwrap(),
        f64::INFINITY
    );
}

#[test]
fn the_supercharger_shifts_speed_with_the_ambient_pressure() {
    let mut fdm = load(&engine(BOOST)).unwrap();
    let b = &piston(&fdm).boost;
    assert!(b.boosted && b.speeds == 2 && !b.b_manual);
    assert_eq!(b.switch_hysteresis, 1000.0);
    assert_eq!(
        b.switch_pressure[0],
        std_pressure_100k(9000.0) * crate::units::PSFTOPA
    );
    fdm.set_property("ic/h-sl-ft", 15000.0).unwrap();
    fdm.set_property("ic/u-fps", 300.0).unwrap();
    fdm.run_ic().unwrap();
    assert_eq!(
        fdm.get_property("propulsion/engine/boost-speed").unwrap(),
        1.0
    );
}

#[test]
fn the_standard_pressure_polynomial_is_capped_at_100000_ft() {
    assert_eq!(std_pressure_100k(0.0), 2116.217);
    assert_eq!(std_pressure_100k(150000.0), std_pressure_100k(100000.0));
}
