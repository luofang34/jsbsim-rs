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
fn load(propulsion: &str) -> Result<Fdm, Error> {
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
fn engines_and_function_grains_are_refused_by_name() {
    let engine = load("<propulsion><engine file=\"x\"/></propulsion>")
        .err()
        .unwrap();
    assert!(matches!(engine, Error::Unsupported { construct, .. } if construct.contains("engine")));
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
