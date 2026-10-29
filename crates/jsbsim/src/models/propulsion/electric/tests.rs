// SPDX-License-Identifier: LGPL-2.1-or-later
use super::Electric;
use crate::fdm::Fdm;
use crate::models::propulsion::engine::EngineType;
use crate::models::propulsion::families::EngineKind;
use crate::models::propulsion::tests::load;
use crate::units::HPTOFTLBSSEC;

#[cfg(test)]
fn engine(power: &str) -> String {
    format!(
        r#"<propulsion>
  <engine>
    <electric_engine name="e">{power}</electric_engine>
    <thruster>
      <location unit="IN"><x>-40</x><y>0</y><z>0</z></location>
      <direct name="d"/>
    </thruster>
  </engine>
</propulsion>"#
    )
}

#[cfg(test)]
fn electric(fdm: &Fdm) -> &Electric {
    match &fdm.models.propulsion.engines[0].kind {
        EngineKind::Electric(e) => e,
        _ => panic!("not an electric engine"),
    }
}

#[test]
fn the_rated_power_defaults_to_one_horsepower() {
    let fdm = load(&engine("")).unwrap();
    assert_eq!(electric(&fdm).power_watts, 745.7);
    assert_eq!(
        fdm.models.propulsion.engines[0].base.engine_type,
        EngineType::Electric
    );
    let fdm = load(&engine(r#"<power unit="WATTS">1491.4</power>"#)).unwrap();
    assert_eq!(electric(&fdm).power_watts, 1491.4);
}

#[test]
fn the_power_is_the_rated_power_times_the_throttle_and_burns_no_fuel() {
    let mut fdm = load(&engine(r#"<power unit="WATTS">7457</power>"#)).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("fcs/throttle-cmd-norm", 0.5).unwrap();
    fdm.run().unwrap();
    fdm.run().unwrap();
    let hp = fdm.get_property("propulsion/engine[0]/power-hp").unwrap();
    assert_eq!(hp, 7457.0 * 0.5 / 745.7);
    let thrust = fdm.get_property("propulsion/engine[0]/thrust-lbs").unwrap();
    assert_eq!(thrust, hp * HPTOFTLBSSEC);
    assert_eq!(
        fdm.get_property("propulsion/engine[0]/fuel-used-lbs")
            .unwrap(),
        0.0
    );
}
