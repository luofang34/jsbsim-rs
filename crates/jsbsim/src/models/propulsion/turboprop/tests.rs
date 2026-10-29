// SPDX-License-Identifier: LGPL-2.1-or-later
use super::{Phase, PowerVc, TurboProp, exp_seek};
use crate::Error;
use crate::fdm::Fdm;
use crate::models::propulsion::families::EngineKind;
use crate::models::propulsion::tests::load;

const TABLES: &str = r#"
      <table name="EnginePowerRPM_N1" type="internal">
        <tableData>
                0     60    100
          0     0     400   1000
          5000  0     400   1000
        </tableData>
      </table>
      <table name="ITT_N1" type="internal">
        <tableData>
                0     1
          0     0     0
          100   500   700
        </tableData>
      </table>"#;

#[cfg(test)]
fn engine(vc: &str) -> String {
    format!(
        r#"<propulsion>
  <tank type="FUEL">
    <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
    <capacity>2000</capacity><contents>2000</contents>
  </tank>
  <engine>
    <feed>0</feed>
    <turboprop_engine name="tp">
      <milthrust>2000</milthrust><idlen1>60</idlen1><maxn1>100</maxn1>
      <maxpower>800</maxpower><psfc>0.6</psfc><startern1>25</startern1>
      <betarangeend>40</betarangeend><reversemaxpower>50</reversemaxpower>
      {TABLES}
      {vc}
    </turboprop_engine>
    <thruster>
      <location unit="IN"><x>-40</x><y>0</y><z>0</z></location>
      <propeller name="p">
        <ixx>5</ixx><diameter unit="IN">100</diameter><numblades>3</numblades>
        <gearratio>10</gearratio>
        <table name="C_THRUST" type="internal">
          <tableData>
            0.0 0.07
            2.0 -0.07
          </tableData>
        </table>
        <table name="C_POWER" type="internal">
          <tableData>
            0.0 0.06
            2.0 0.05
          </tableData>
        </table>
      </propeller>
    </thruster>
  </engine>
</propulsion>"#
    )
}

const VC_FUNCTION: &str = r#"<function name="EnginePowerVC"><value>1.0</value></function>"#;

const VC_TABLE: &str = r#"<table name="EnginePowerVC">
        <independentVar lookup="row">atmosphere/P-psf</independentVar>
        <tableData>
          0      1.0
          3000   1.0
        </tableData>
      </table>"#;

#[cfg(test)]
fn turboprop(fdm: &Fdm) -> &TurboProp {
    match &fdm.models.propulsion.engines[0].kind {
        EngineKind::TurboProp(t) => t,
        _ => panic!("not a turboprop"),
    }
}

#[test]
fn exp_seek_lags_toward_the_target() {
    let up = exp_seek(0.5, 10.0, 20.0, 1.0, 2.0);
    assert_eq!(up, 10.0 * (1.0 - crate::elementary::exp(-0.5)) + 10.0);
    let down = exp_seek(0.5, 20.0, 10.0, 1.0, 2.0);
    assert_eq!(down, 10.0 * crate::elementary::exp(-0.25) + 10.0);
    assert_eq!(exp_seek(0.5, 3.0, 3.0, 1.0, 2.0), 3.0);
}

#[test]
fn engine_power_vc_is_a_function_or_a_table_bound_under_the_engine() {
    let fdm = load(&engine(VC_FUNCTION)).unwrap();
    assert!(matches!(
        turboprop(&fdm).config.engine_power_vc,
        PowerVc::Function(_)
    ));
    let mut fdm = load(&engine(VC_TABLE)).unwrap();
    assert!(matches!(
        turboprop(&fdm).config.engine_power_vc,
        PowerVc::Table(_)
    ));
    let vc = fdm
        .get_property("propulsion/engine[0]/EnginePowerVC")
        .unwrap();
    assert_eq!(vc, 1.0);
    assert_eq!(turboprop(&fdm).config.beta_range_throttle_end, 0.4);
}

#[test]
fn a_turboprop_without_its_tables_is_refused() {
    assert!(matches!(load(&engine("")), Err(Error::Model { .. })));
    let text = engine(VC_FUNCTION).replace(TABLES, "");
    assert!(matches!(load(&text), Err(Error::Model { .. })));
}

#[test]
fn a_running_engine_turns_the_propeller_and_burns_fuel() {
    let mut fdm = load(&engine(VC_FUNCTION)).unwrap();
    fdm.set_property("ic/h-sl-ft", 5000.0).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("propulsion/set-running", -1.0).unwrap();
    assert_eq!(turboprop(&fdm).state.phase, Phase::Run);
    fdm.run().unwrap();
    let t = turboprop(&fdm);
    assert!(t.state.n1 > 60.0, "{:?}", t.state);
    assert!(t.state.hp > 0.0);
    assert!(
        fdm.get_property("propulsion/engine[0]/propeller-rpm")
            .unwrap()
            > 0.0
    );
    assert!(
        fdm.get_property("propulsion/engine[0]/fuel-flow-rate-pps")
            .unwrap()
            > 0.0
    );
    fdm.set_property("propulsion/cutoff_cmd", 1.0).unwrap();
    fdm.run().unwrap();
    assert_eq!(turboprop(&fdm).state.phase, Phase::Off);
}

#[test]
fn reversing_idles_in_the_beta_range_and_scales_beyond_it() {
    let mut fdm = load(&engine(VC_FUNCTION)).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("propulsion/engine[0]/reverser", 1.0)
        .unwrap();
    fdm.set_property("fcs/throttle-cmd-norm", 0.3).unwrap();
    fdm.run().unwrap();
    assert_eq!(turboprop(&fdm).state.throttle_pos, 0.0);
    fdm.set_property("fcs/throttle-cmd-norm", 1.0).unwrap();
    fdm.run().unwrap();
    assert_eq!(turboprop(&fdm).state.throttle_pos, 0.5);
}
