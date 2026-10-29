// SPDX-License-Identifier: LGPL-2.1-or-later
use super::{Phase, Spool, Turbine, seek};
use crate::Error;
use crate::fdm::Fdm;
use crate::models::propulsion::families::EngineKind;
use crate::models::propulsion::tests::load;

#[cfg(test)]
fn engine(extra: &str, functions: &str) -> String {
    format!(
        r#"<propulsion>
  <tank type="FUEL">
    <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
    <capacity>5000</capacity><contents>5000</contents>
  </tank>
  <engine>
    <feed>0</feed>
    <turbine_engine name="t">
      <milthrust>10000</milthrust><bypassratio>1.0</bypassratio>
      <tsfc>0.6</tsfc><idlen1>30</idlen1><idlen2>60</idlen2>
      <maxn1>100</maxn1><maxn2>100</maxn2>
      {extra}
      <function name="IdleThrust"><value>0.05</value></function>
      <function name="MilThrust"><value>0.9</value></function>
      {functions}
    </turbine_engine>
    <thruster>
      <location unit="IN"><x>-40</x><y>0</y><z>0</z></location>
      <nozzle name="n"><area unit="FT2">0.5</area></nozzle>
    </thruster>
  </engine>
</propulsion>"#
    )
}

#[cfg(test)]
fn turbine(fdm: &Fdm) -> &Turbine {
    match &fdm.models.propulsion.engines[0].kind {
        EngineKind::Turbine(t) => t,
        _ => panic!("not a turbine"),
    }
}

#[test]
fn seek_moves_toward_the_target_without_passing_it() {
    assert_eq!(seek(0.5, 10.0, 20.0, 4.0, 1.0), 12.0);
    assert_eq!(seek(0.5, 10.0, 11.0, 4.0, 1.0), 11.0);
    assert_eq!(seek(0.5, 10.0, 0.0, 4.0, 1.0), 9.5);
    assert_eq!(seek(0.5, 10.0, 9.9, 4.0, 1.0), 9.9);
    assert_eq!(seek(0.5, 10.0, 10.0, 4.0, 1.0), 10.0);
}

#[test]
fn the_engine_functions_are_renamed_under_the_engine_and_looked_up() {
    let spool = r#"<function name="N1SpoolUp"><value>7</value></function>"#;
    let fdm = load(&engine("", spool)).unwrap();
    let t = turbine(&fdm);
    assert!(matches!(t.lookups.n1_spool_up, Spool::Function(_)));
    assert_eq!(t.lookups.n2_spool_up, Spool::Default { delay: 22.5 });
    assert_eq!(t.lookups.n1_spool_down, Spool::Default { delay: 54.0 });
    assert!(t.lookups.aug_thrust.is_none());
    assert!(
        fdm.node("propulsion/engine[0]/IdleThrust")
            .unwrap()
            .is_some()
    );
    assert!(
        fdm.node("propulsion/engine[0]/N1SpoolUp")
            .unwrap()
            .is_some()
    );
    assert_eq!(t.config.idle_ff, libm::pow(10000.0, 0.2) * 107.0);
}

#[test]
fn a_turbine_without_idle_thrust_is_refused() {
    let text = engine("", "").replace(
        r#"<function name="IdleThrust"><value>0.05</value></function>"#,
        "",
    );
    assert!(matches!(load(&text), Err(Error::Model { .. })));
}

#[test]
fn a_tsfc_function_is_unsupported() {
    let text = engine("", "").replace(
        "<tsfc>0.6</tsfc>",
        "<tsfc><product><value>0.6</value><value>1</value></product></tsfc>",
    );
    assert!(matches!(load(&text), Err(Error::Unsupported { .. })));
}

#[test]
fn the_ties_read_and_write_the_engine() {
    let mut fdm = load(&engine("<bleed>0.03</bleed>", "")).unwrap();
    assert_eq!(
        fdm.get_property("propulsion/engine[0]/bleed-factor")
            .unwrap(),
        0.03
    );
    fdm.set_property("propulsion/engine[0]/MaxN2", 102.0)
        .unwrap();
    assert_eq!(turbine(&fdm).state.max_n2, 102.0);
    assert_eq!(fdm.get_property("propulsion/engine[0]/atsfc").unwrap(), 1.7);
    assert!(!fdm.set_property("propulsion/engine[0]/tsfc", 1.0).unwrap());
    assert_eq!(fdm.get_property("propulsion/cutoff_cmd").unwrap(), 1.0);
}

#[test]
fn a_running_engine_spools_to_the_throttle_and_burns_fuel() {
    let mut fdm = load(&engine("", "")).unwrap();
    fdm.set_property("ic/h-sl-ft", 10000.0).unwrap();
    fdm.run_ic().unwrap();
    assert_eq!(turbine(&fdm).state.phase, Phase::Trim);
    fdm.set_property("propulsion/set-running", -1.0).unwrap();
    fdm.run().unwrap();
    let t = turbine(&fdm);
    assert_eq!(t.state.phase, Phase::Run);
    assert!(!t.flags.cutoff);
    // Full throttle: N1 and N2 settle near their maxima in the steady
    // state, which stops once the thrust changes by less than 1e-4.
    assert!(t.state.n1 > 99.0 && t.state.n2 > 99.0, "{:?}", t.state);
    let thrust = fdm.get_property("propulsion/engine[0]/thrust-lbs").unwrap();
    assert!(thrust > 5000.0, "{thrust}");
    assert!(
        fdm.get_property("propulsion/engine[0]/fuel-flow-rate-pps")
            .unwrap()
            > 0.0
    );
    fdm.set_property("propulsion/cutoff_cmd", 1.0).unwrap();
    fdm.run().unwrap();
    assert_eq!(turbine(&fdm).state.phase, Phase::Off);
    assert_eq!(
        fdm.get_property("propulsion/engine[0]/thrust-lbs").unwrap(),
        0.0
    );
}

#[test]
fn the_starter_spins_the_engine_up_and_it_lights_once_fuel_flows() {
    let mut fdm = load(&engine("", "")).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("propulsion/starter_cmd", 1.0).unwrap();
    fdm.run().unwrap();
    assert_eq!(turbine(&fdm).state.phase, Phase::SpinUp);
    for _ in 0..1200 {
        fdm.run().unwrap();
    }
    assert!(turbine(&fdm).state.n2 > 15.0);
    fdm.set_property("propulsion/cutoff_cmd", 0.0).unwrap();
    fdm.run().unwrap();
    assert_eq!(turbine(&fdm).state.phase, Phase::Start);
    for _ in 0..12000 {
        fdm.run().unwrap();
    }
    let t = turbine(&fdm);
    assert_eq!(t.state.phase, Phase::Run);
    assert!(fdm.models.propulsion.engines[0].base.running);
}
