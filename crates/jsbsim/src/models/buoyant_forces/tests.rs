// SPDX-License-Identifier: LGPL-2.1-or-later
use std::f64::consts::PI;

use super::*;
use crate::fdm::Fdm;
use crate::models::atmosphere::{STD_DAY_SL_PRESSURE, STD_DAY_SL_TEMPERATURE};
use crate::xml::parse_document;

fn element(xml: &str) -> Element {
    parse_document(xml.as_bytes(), "test.xml").unwrap()
}

const SL: Inputs = Inputs {
    pressure: STD_DAY_SL_PRESSURE,
    temperature: STD_DAY_SL_TEMPERATURE,
    density: 0.0023769,
    gravity: 32.17,
};

#[test]
fn an_ellipsoid_cell_fills_its_volume_at_ambient_pressure() {
    let el = element(
        r#"<gas_cell type="HELIUM">
  <location unit="M"><x>0</x><y>0</y><z>0</z></location>
  <x_radius unit="FT">10</x_radius><y_radius unit="FT">5</y_radius>
  <z_radius unit="FT">4</z_radius>
</gas_cell>"#,
    );
    let cell = GasCell::load(&el, &SL).unwrap();
    let volume = 4.0 * PI * 10.0 * 5.0 * 4.0 / 3.0;
    assert_eq!(cell.envelope.max_volume, volume);
    let contents = SL.pressure * volume / (GAS_R * SL.temperature);
    assert_eq!(cell.contents, contents);
    assert_eq!(cell.pressure, SL.pressure);
    assert_eq!(cell.mass, contents * 0.00027409);
}

#[test]
fn a_fullness_above_what_the_overpressure_allows_is_clipped() {
    let el = element(
        r#"<gas_cell type="HYDROGEN">
  <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
  <x_width unit="FT">20</x_width><y_radius unit="FT">5</y_radius>
  <z_radius unit="FT">5</z_radius>
  <max_overpressure unit="LBS/FT2">10</max_overpressure>
  <fullness>2.0</fullness>
</gas_cell>"#,
    );
    let cell = GasCell::load(&el, &SL).unwrap();
    let volume = PI * 5.0 * 5.0 * 20.0;
    assert_eq!(cell.envelope.max_volume, volume);
    assert_eq!(cell.pressure, SL.pressure + 10.0);
    assert_eq!(
        cell.contents,
        (SL.pressure + 10.0) * volume / (GAS_R * SL.temperature)
    );
}

#[test]
fn a_cell_without_location_or_shape_is_refused() {
    let no_location = element(r#"<gas_cell><x_radius>1</x_radius></gas_cell>"#);
    assert!(matches!(
        GasCell::load(&no_location, &SL),
        Err(Error::Model { .. })
    ));
    let no_shape = element(
        r#"<gas_cell><location unit="IN"><x>0</x><y>0</y><z>0</z></location>
  <x_radius>1</x_radius></gas_cell>"#,
    );
    assert!(matches!(
        GasCell::load(&no_shape, &SL),
        Err(Error::Model { .. })
    ));
}

const BALLOON: &str = r#"<fdm_config name="balloon">
  <metrics/>
  <mass_balance>
    <ixx>10</ixx><iyy>10</iyy><izz>10</izz><emptywt>10</emptywt>
    <location name="CG" unit="IN"><x>0</x><y>0</y><z>0</z></location>
  </mass_balance>
  <ground_reactions/>
  <buoyant_forces>
    <property value="0">test/blower</property>
    <gas_cell type="HELIUM">
      <location unit="IN"><x>0</x><y>0</y><z>-120</z></location>
      <x_radius unit="FT">5</x_radius><y_radius unit="FT">5</y_radius>
      <z_radius unit="FT">5</z_radius>
      <valve_coefficient unit="FT4*SEC/SLUG">100</valve_coefficient>
      <ballonet type="AIR">
        <location unit="IN"><x>0</x><y>0</y><z>-60</z></location>
        <x_radius unit="FT">1</x_radius><y_radius unit="FT">1</y_radius>
        <z_radius unit="FT">1</z_radius>
        <blower_input>
          <function><property>test/blower</property></function>
        </blower_input>
      </ballonet>
    </gas_cell>
  </buoyant_forces>
</fdm_config>"#;

fn balloon() -> Fdm {
    let mut r = crate::MemoryResolver::new();
    r.insert("aircraft/balloon/balloon.xml", BALLOON);
    let mut fdm = Fdm::new().unwrap();
    fdm.set_property("simulation/models/FGAerodynamics/enabled", 0.0)
        .unwrap();
    fdm.load_model(&r, "aircraft", "balloon").unwrap();
    fdm.set_property("ic/h-sl-ft", 1000.0).unwrap();
    fdm.run_ic().unwrap();
    fdm
}

#[test]
fn the_cell_lifts_by_the_air_it_displaces_and_weighs_its_gas() {
    let mut fdm = balloon();
    fdm.run().unwrap();
    let cell = fdm.models.buoyant_forces.cells[0].clone();
    let input = fdm.models.buoyant_forces.input;
    assert_eq!(cell.buoyancy, cell.volume * input.density * input.gravity);
    let lift = fdm.get_property("forces/fbz-buoyancy-lbs").unwrap();
    assert!(lift < 0.0, "level, the buoyancy points up: {lift}");
    assert_eq!(
        fdm.get_property("buoyant_forces/gas-cell/buoyancy-lbs")
            .unwrap(),
        cell.buoyancy
    );
    let gas = cell.contents * 0.00027409 + cell.ballonets[0].mass();
    assert_eq!(fdm.models.buoyant_forces.gas_mass(), gas);
    // The mass balance weighs the gas from the next frame on.
    fdm.run().unwrap();
    let weight = fdm.get_property("inertia/weight-lbs").unwrap();
    assert!(weight > 10.0, "{weight}");
}

#[test]
fn the_valve_vents_gas_and_the_blower_inflates_the_ballonet() {
    let mut fdm = balloon();
    fdm.run().unwrap();
    let before = fdm.models.buoyant_forces.cells[0].clone();
    // The cell's gas is above ambient pressure only with a head of gas
    // lighter than air, which the valve then lets out.
    fdm.set_property("buoyant_forces/gas-cell/valve_open", 1.0)
        .unwrap();
    fdm.set_property("test/blower", 10.0).unwrap();
    for _ in 0..10 {
        fdm.run().unwrap();
    }
    let after = fdm.models.buoyant_forces.cells[0].clone();
    assert!(after.contents < before.contents);
    assert!(after.ballonets[0].contents > before.ballonets[0].contents);
    assert_eq!(
        fdm.get_property("buoyant_forces/gas-cell/ballonet/contents-mol")
            .unwrap(),
        after.ballonets[0].contents
    );
}

#[test]
fn max_volume_is_read_only() {
    let mut fdm = balloon();
    let path = "buoyant_forces/gas-cell/max_volume-ft3";
    let volume = fdm.get_property(path).unwrap();
    assert!(!fdm.set_property(path, 1.0).unwrap());
    assert_eq!(fdm.get_property(path).unwrap(), volume);
}
