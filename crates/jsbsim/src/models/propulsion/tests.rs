// SPDX-License-Identifier: LGPL-2.1-or-later
use crate::fdm::Fdm;
use crate::units::LBTOSLUG;
use crate::{Error, MemoryResolver};

#[cfg(test)]
fn aircraft(propulsion: &str) -> String {
    format!(
        r#"<fdm_config name="t">
  <metrics/>
  <mass_balance>
    <ixx>100</ixx><iyy>200</iyy><izz>300</izz>
    <emptywt>1000</emptywt>
    <location name="CG" unit="IN"><x>0</x><y>0</y><z>0</z></location>
  </mass_balance>
  <ground_reactions/>
  {propulsion}
  <aerodynamics/>
</fdm_config>"#
    )
}

#[cfg(test)]
pub(super) fn load(propulsion: &str) -> Result<Fdm, Error> {
    let mut r = MemoryResolver::new();
    r.insert("aircraft/t/t.xml", aircraft(propulsion));
    let mut fdm = Fdm::new()?;
    fdm.load_model(&r, "aircraft", "t")?;
    Ok(fdm)
}

const TWO_TANKS: &str = r#"<propulsion>
  <tank type="FUEL">
    <location unit="IN"><x>12</x><y>0</y><z>0</z></location>
    <radius unit="FT">1</radius>
    <capacity>100</capacity><contents>80</contents>
    <unusable-volume>1</unusable-volume>
    <standpipe>30</standpipe>
  </tank>
  <tank type="OXIDIZER">
    <location unit="IN"><x>-24</x><y>0</y><z>0</z></location>
    <capacity>50</capacity><contents>10</contents>
  </tank>
  <dump-rate>60</dump-rate>
  <refuel-rate>120</refuel-rate>
</propulsion>"#;

#[test]
fn draining_stops_at_the_unusable_contents_and_filling_at_capacity() {
    let mut fdm = load(TWO_TANKS).unwrap();
    let tank = &mut fdm.models.propulsion.tanks[0];
    assert_eq!(tank.unusable(), 6.6);
    assert_eq!(tank.drain(70.0), 10.0);
    assert_eq!(tank.drain(10.0), 6.6);
    assert_eq!(tank.contents, 6.6);
    assert_eq!(tank.fill(200.0), 106.6);
    assert_eq!((tank.contents, tank.pct_full), (100.0, 100.0));
}

#[test]
fn halving_the_contents_halves_a_liquid_tanks_inertia() {
    let mut fdm = load(TWO_TANKS).unwrap();
    let ixx = fdm
        .get_property("propulsion/tank/local-ixx-slug_ft2")
        .unwrap();
    assert!((ixx - 80.0 * LBTOSLUG * 0.4).abs() < 1e-15);
    fdm.set_property("propulsion/tank/contents-lbs", 40.0)
        .unwrap();
    let half = fdm
        .get_property("propulsion/tank/local-ixx-slug_ft2")
        .unwrap();
    assert!((half - 0.5 * ixx).abs() < 1e-15);
}

#[test]
fn the_tanks_load_the_mass_balance() {
    let mut fdm = load(TWO_TANKS).unwrap();
    fdm.set_property("ic/h-sl-ft", 1000.0).unwrap();
    fdm.run_ic().unwrap();
    assert_eq!(fdm.get_property("inertia/weight-lbs").unwrap(), 1090.0);
    let cg = (80.0 * 12.0 - 10.0 * 24.0) / 1090.0;
    let got = fdm.get_property("inertia/cg-x-in").unwrap();
    assert!((got - cg).abs() < 1e-12, "{got} vs {cg}");
}

#[test]
fn refuel_shares_the_rate_and_dump_stops_at_the_standpipe() {
    let mut fdm = load(TWO_TANKS).unwrap();
    let p = &mut fdm.models.propulsion;
    p.do_refuel(10.0);
    assert_eq!(p.tanks[0].contents, 90.0);
    assert_eq!(p.tanks[1].contents, 20.0);
    p.dump_fuel(10.0);
    assert_eq!(p.tanks[0].contents, 85.0);
    assert_eq!(p.tanks[1].contents, 15.0);
    p.dump_fuel(100.0);
    assert_eq!(p.tanks[1].contents, 0.0);
    p.dump_fuel(1000.0);
    assert_eq!(p.tanks[0].contents, 6.6);
}

#[test]
fn a_cylindrical_grain_full_has_the_inertia_of_a_thick_tube() {
    let fdm = load(
        r#"<propulsion><tank type="FUEL">
      <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
      <radius unit="FT">6.0</radius>
      <grain_config type="CYLINDRICAL">
        <length unit="FT">124.0</length>
        <bore_diameter unit="FT">2.0</bore_diameter>
      </grain_config>
      <capacity>1114092</capacity><contents>1114092</contents>
    </tank></propulsion>"#,
    )
    .unwrap();
    let tank = &fdm.models.propulsion.tanks[0];
    let ixx = 0.5 * 1114092.0 * LBTOSLUG * (36.0 + 1.0);
    assert!((tank.ixx - ixx).abs() < 1e-9 * ixx, "{} vs {ixx}", tank.ixx);
}

#[test]
fn reset_restores_the_initial_contents() {
    let mut fdm = load(TWO_TANKS).unwrap();
    fdm.set_property("propulsion/tank[1]/contents-lbs", 0.0)
        .unwrap();
    fdm.set_property("propulsion/tank[1]/priority", 0.0)
        .unwrap();
    assert!(!fdm.models.propulsion.tanks[1].selected);
    fdm.models.propulsion.init_model();
    let tank = &fdm.models.propulsion.tanks[1];
    assert_eq!(
        (tank.contents, tank.priority, tank.selected),
        (10.0, 1, true)
    );
}

#[test]
fn engine_and_thruster_types_are_refused_by_name() {
    let propeller = r#"<propulsion><engine>
      <test_engine><sl-fuel-flow>1</sl-fuel-flow></test_engine>
      <thruster><rotor/></thruster>
    </engine></propulsion>"#;
    let err = load(propeller).err().unwrap();
    assert!(matches!(&err, Error::Unsupported { construct, .. } if construct == "<rotor>"));
    let unknown = "<propulsion><engine><steam/><thruster/></engine></propulsion>";
    assert!(matches!(load(unknown).err(), Some(Error::Model { .. })));
    let no_thruster = "<propulsion><engine><test_engine/></engine></propulsion>";
    assert!(matches!(load(no_thruster).err(), Some(Error::Model { .. })));
    let grain = load(
        r#"<propulsion><tank type="FUEL">
      <grain_config type="FUNCTION"/><capacity>1</capacity>
    </tank></propulsion>"#,
    )
    .err()
    .unwrap();
    assert!(
        matches!(grain, Error::Unsupported { construct, .. } if construct.contains("FUNCTION"))
    );
}

/// A rocket fed by two fuel tanks of priorities 1 and 2 and an oxidizer
/// tank, and a second engine fed by a tank that does not exist.
const ROCKET: &str = r#"<propulsion>
  <tank type="FUEL">
    <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
    <capacity>100</capacity><contents>12</contents>
  </tank>
  <tank type="FUEL">
    <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
    <capacity>100</capacity><contents>100</contents><priority>2</priority>
  </tank>
  <tank type="OXIDIZER">
    <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
    <capacity>100</capacity><contents>100</contents>
  </tank>
  <engine>
    <feed>0</feed><feed>1</feed><feed>2</feed>
    <test_engine name="r0"><sl-fuel-flow>120</sl-fuel-flow></test_engine>
    <thruster><test_thruster name="t0"/></thruster>
  </engine>
</propulsion>"#;

#[cfg(test)]
fn running_rocket() -> Fdm {
    let mut fdm = load(ROCKET).unwrap();
    fdm.set_property("ic/h-sl-ft", 10000.0).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("propulsion/set-running", -1.0).unwrap();
    fdm
}

#[test]
fn starting_an_engine_reaches_a_steady_thrust_and_sums_its_force() {
    let mut fdm = running_rocket();
    assert_eq!(
        fdm.get_property("propulsion/engine/set-running").unwrap(),
        1.0
    );
    assert_eq!(
        fdm.get_property("propulsion/engine/thrust-lbs").unwrap(),
        1000.0
    );
    assert_eq!(fdm.get_property("forces/fbx-prop-lbs").unwrap(), 1000.0);
    // The thruster at (10, 0, 12) in pushes forward 1 ft above the CG,
    // pitching the nose down.
    let pitch = fdm.get_property("moments/m-prop-lbsft").unwrap();
    assert!((pitch + 1000.0).abs() < 1e-9, "{pitch}");
    let calculations = match &fdm.models.propulsion.engines[0].kind {
        super::families::EngineKind::Test(e) => e.calculations,
        _ => panic!("not the test engine"),
    };
    assert!(calculations > 120, "{calculations}");
}

#[test]
fn fuel_is_drawn_by_priority_and_oxidizer_with_it() {
    let mut fdm = running_rocket();
    let dt = fdm.models.exec.dt;
    fdm.run().unwrap();
    let p = &fdm.models.propulsion;
    assert!((p.tanks[0].contents - (12.0 - 120.0 * dt)).abs() < 1e-12);
    assert_eq!(p.tanks[1].contents, 100.0);
    assert!((p.tanks[2].contents - (100.0 - 60.0 * dt)).abs() < 1e-12);
    for _ in 0..20 {
        fdm.run().unwrap();
    }
    let p = &fdm.models.propulsion;
    assert_eq!(p.tanks[0].contents, 0.0);
    assert!(p.tanks[1].contents < 100.0);
    let used = fdm.get_property("propulsion/engine/fuel-used-lbs").unwrap();
    let total = fdm.get_property("propulsion/total-fuel-lbs").unwrap();
    assert!((used + total - 112.0).abs() < 1e-9, "{used} + {total}");
    assert_eq!(
        fdm.get_property("propulsion/engine/fuel-flow-rate-pps")
            .unwrap(),
        120.0
    );
}

#[test]
fn an_engine_without_oxidizer_starves_and_freezing_burns_nothing() {
    let mut fdm = running_rocket();
    fdm.set_property("propulsion/tank[2]/contents-lbs", 0.0)
        .unwrap();
    fdm.run().unwrap();
    assert!(fdm.models.propulsion.engines[0].base.starved);
    assert_eq!(fdm.models.propulsion.tanks[0].contents, 12.0);
    fdm.set_property("propulsion/tank[2]/contents-lbs", 50.0)
        .unwrap();
    fdm.set_property("propulsion/fuel_freeze", 1.0).unwrap();
    fdm.run().unwrap();
    assert_eq!(fdm.models.propulsion.tanks[0].contents, 12.0);
}

#[test]
fn starting_a_missing_engine_fails() {
    let mut fdm = load(ROCKET).unwrap();
    assert!(fdm.set_property("propulsion/set-running", 3.0).is_err());
}

#[test]
fn an_engine_fed_by_a_missing_tank_fails_to_run() {
    let rocket = ROCKET.replace("<feed>2</feed>", "<feed>7</feed>");
    let mut fdm = load(&rocket).unwrap();
    fdm.set_property("ic/h-sl-ft", 10000.0).unwrap();
    assert!(matches!(fdm.run_ic(), Err(Error::Model { .. })));
}

#[test]
fn each_engine_gets_throttle_properties_whose_positions_follow_the_commands() {
    let mut fdm = running_rocket();
    for (cmd, pos) in [
        ("throttle-cmd-norm", "throttle-pos-norm"),
        ("mixture-cmd-norm", "mixture-pos-norm"),
        ("advance-cmd-norm", "advance-pos-norm"),
        ("feather-cmd-norm", "feather-pos-norm"),
    ] {
        fdm.set_property(&format!("fcs/{cmd}[0]"), 0.75).unwrap();
        fdm.run().unwrap();
        let expected = if cmd.starts_with("feather") {
            1.0
        } else {
            0.75
        };
        let got = fdm.get_property(&format!("fcs/{pos}")).unwrap();
        assert_eq!(got, expected, "{pos}");
    }
    assert!(fdm.node("fcs/throttle-cmd-norm[1]").unwrap().is_none());
    match &fdm.models.propulsion.engines[0].kind {
        super::families::EngineKind::Test(e) => assert_eq!(e.throttle, 0.75),
        _ => panic!("not the test engine"),
    }
    fdm.models.fcs.init_model();
    let t = &fdm.models.fcs.throttles;
    assert_eq!((t.throttle_cmd[0], t.throttle_pos[0]), (0.0, 0.0));
    assert_eq!((t.prop_advance_cmd[0], t.prop_feather_cmd[0]), (0.75, true));
    assert_eq!((t.prop_advance[0], t.prop_feather[0]), (0.0, false));
}

#[test]
fn starting_an_engine_sets_its_fcs_mixture() {
    let fdm = running_rocket();
    let t = &fdm.models.fcs.throttles;
    assert_eq!((t.mixture_cmd[0], t.mixture_pos[0]), (1.0, 1.0));
    assert_eq!(t.throttle_cmd[0], 0.0);
    let input = &fdm.models.propulsion.input;
    assert_eq!((input.throttle_cmd[0], input.mixture_pos[0]), (1.0, 1.0));
}

#[test]
fn run_ic_starts_the_engines_the_initial_conditions_name() {
    let mut fdm = load(ROCKET).unwrap();
    let mut r = MemoryResolver::new();
    let ic = "<initialize><altitude unit=\"FT\">10000</altitude><running>0</running></initialize>";
    r.insert("aircraft/t/running.xml", ic.as_bytes().to_vec());
    assert!(
        fdm.load_initial_conditions(&r, "aircraft/t", "running")
            .unwrap()
    );
    assert_eq!(
        fdm.get_property("propulsion/engine/set-running").unwrap(),
        0.0
    );
    fdm.run_ic().unwrap();
    assert_eq!(
        fdm.get_property("propulsion/engine/set-running").unwrap(),
        1.0
    );
    // A reset runs the initial conditions again, which start it again.
    fdm.set_property("propulsion/engine/set-running", 0.0)
        .unwrap();
    fdm.reset_to_initial_conditions(0).unwrap();
    assert_eq!(
        fdm.get_property("propulsion/engine/set-running").unwrap(),
        1.0
    );
}
