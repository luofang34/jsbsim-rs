// SPDX-License-Identifier: LGPL-2.1-or-later
use super::Rocket;
use crate::Error;
use crate::fdm::Fdm;
use crate::models::propulsion::engine::EngineType;
use crate::models::propulsion::families::EngineKind;
use crate::models::propulsion::tests::load;

const NOZZLE: &str = r#"<thruster>
      <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
      <nozzle name="n"><area unit="FT2">2</area></nozzle>
    </thruster>"#;

#[cfg(test)]
fn liquid(engine: &str) -> String {
    format!(
        r#"<propulsion>
  <tank type="FUEL">
    <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
    <capacity>1000</capacity><contents>1000</contents>
  </tank>
  <tank type="OXIDIZER">
    <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
    <capacity>1000</capacity><contents>1000</contents>
  </tank>
  <engine>
    <feed>0</feed><feed>1</feed>
    <rocket_engine name="r">{engine}</rocket_engine>
    {NOZZLE}
  </engine>
</propulsion>"#
    )
}

#[cfg(test)]
fn solid(variation: &str) -> String {
    format!(
        r#"<propulsion>
  <tank type="FUEL">
    <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
    <radius unit="IN">10</radius>
    <grain_config type="CYLINDRICAL">
      <length unit="IN">100</length><bore_diameter unit="IN">4</bore_diameter>
    </grain_config>
    <capacity>100</capacity><contents>100</contents>
  </tank>
  <engine>
    <feed>0</feed>
    <rocket_engine name="s">
      <isp>200</isp><builduptime>0.1</builduptime>{variation}
      <thrust_table type="internal">
        <tableData>
          0    10000
          100  10000
        </tableData>
      </thrust_table>
    </rocket_engine>
    {NOZZLE}
  </engine>
</propulsion>"#
    )
}

#[cfg(test)]
fn rocket(fdm: &Fdm) -> &Rocket {
    match &fdm.models.propulsion.engines[0].kind {
        EngineKind::Rocket(r) => r,
        _ => panic!("not a rocket"),
    }
}

const XLR99: &str = r#"<isp>260</isp><minthrottle>0.4</minthrottle>
      <slfuelflowmax unit="LBS/SEC">91.5</slfuelflowmax>
      <sloxiflowmax unit="LBS/SEC">105.2</sloxiflowmax>"#;

#[test]
fn a_rocket_needs_a_specific_impulse() {
    assert!(matches!(load(&liquid("")), Err(Error::Model { .. })));
}

#[test]
fn the_flows_set_the_mixture_ratio_and_every_rocket_gets_the_liquid_ties() {
    let fdm = load(&liquid(XLR99)).unwrap();
    let r = rocket(&fdm);
    assert_eq!(r.mxr, 105.2 / 91.5);
    assert_eq!(r.prop_flow_max, 105.2 + 91.5);
    assert_eq!(fdm.models.propulsion.engines[0].base.min_throttle, 0.4);
    assert_eq!(
        fdm.models.propulsion.engines[0].base.engine_type,
        EngineType::Rocket
    );
    let mut fdm = load(&solid("")).unwrap();
    let tied = |fdm: &mut Fdm, p: &str| {
        let path = format!("propulsion/engine[0]/{p}");
        fdm.tree().has_node(&path).unwrap()
    };
    for p in ["isp", "mixture-ratio", "oxi-flow-rate-pps", "total-impulse"] {
        assert!(tied(&mut fdm, p), "{p}");
    }
    assert!(!tied(&mut fdm, "thrust-variation_pct"));
}

#[test]
fn a_liquid_rocket_burns_its_propellant_flow_a_step_later_and_flames_out_below_min_throttle() {
    let mut fdm = load(&liquid(XLR99)).unwrap();
    fdm.run_ic().unwrap();
    assert_eq!(rocket(&fdm).vac_thrust, 0.0);
    fdm.set_property("fcs/throttle-cmd-norm", 1.0).unwrap();
    fdm.run().unwrap();
    // The first step burns, the second has the thrust of what it burnt.
    assert_eq!(rocket(&fdm).vac_thrust, 0.0);
    fdm.run().unwrap();
    let r = rocket(&fdm);
    assert!(r.propellant_flow_rate > 196.0, "{}", r.propellant_flow_rate);
    assert_eq!(r.vac_thrust, 260.0 * r.propellant_flow_rate);
    assert!(!r.flameout);
    // Like FGRocket::CalcFuelNeed, nothing is added to the fuel used.
    let used = fdm.get_property("propulsion/engine[0]/fuel-used-lbs");
    assert_eq!(used.unwrap(), 0.0);
    fdm.set_property("fcs/throttle-cmd-norm", 0.3).unwrap();
    fdm.run().unwrap();
    assert!(rocket(&fdm).flameout);
    assert_eq!(rocket(&fdm).vac_thrust, 0.0);
}

#[test]
fn a_solid_rocket_ignites_at_full_throttle_ramps_up_and_burns_on() {
    let mut fdm = load(&solid("<variation><thrust>0.02</thrust></variation>")).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("fcs/throttle-cmd-norm", 0.99).unwrap();
    fdm.run().unwrap();
    assert_eq!(rocket(&fdm).vac_thrust, 0.0);
    fdm.set_property("fcs/throttle-cmd-norm", 1.0).unwrap();
    fdm.run().unwrap();
    // Ignited at burn time 0: the build-up's sine is 0.
    assert_eq!(rocket(&fdm).vac_thrust, 0.0);
    fdm.set_property("fcs/throttle-cmd-norm", 0.0).unwrap();
    fdm.run().unwrap();
    let r = rocket(&fdm);
    let ramp = libm::sin((r.burn_time / 2.0 / 0.1) * std::f64::consts::PI / 2.0);
    assert_eq!(r.vac_thrust, 10000.0 * 1.02 * ramp);
    for _ in 0..240 {
        fdm.run().unwrap();
    }
    let contents = fdm.models.propulsion.tanks[0].contents;
    assert!(contents < 100.0 - 10.0, "{contents}");
}

#[test]
fn nothing_runs_while_integration_is_suspended() {
    let mut fdm = load(&solid("")).unwrap();
    fdm.set_property("fcs/throttle-cmd-norm", 1.0).unwrap();
    fdm.run_ic().unwrap();
    assert_eq!(rocket(&fdm).burn_time, 0.0);
    fdm.models.exec.dt = 0.0;
    fdm.run().unwrap();
    assert_eq!(rocket(&fdm).burn_time, 0.0);
}
