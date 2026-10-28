// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use crate::fdm::Fdm;
use crate::math::Axis;
use crate::xml::parse_document;

fn loaded(xml: &str) -> Fdm {
    let doc = parse_document(xml.as_bytes(), "test.xml").unwrap();
    let mut fdm = Fdm::new().unwrap();
    fdm.load_mass_balance(&doc).unwrap();
    fdm
}

fn run_mass_balance(fdm: &mut Fdm) {
    let m = &mut fdm.models;
    m.mass_balance
        .run(false, m.exec.hold_down, &mut m.propagate);
}

const TWO_MASSES: &str = r#"<mass_balance>
  <ixx>1000</ixx><iyy>2000</iyy><izz>3000</izz><ixz>50</ixz>
  <emptywt>1000</emptywt>
  <location name="CG" unit="IN"><x>100</x><y>0</y><z>0</z></location>
  <pointmass name="pilot">
    <weight>200</weight>
    <location unit="IN"><x>40</x><y>0</y><z>10</z></location>
  </pointmass>
  <pointmass name="tank">
    <form shape="tube"><radius unit="FT">1</radius><length unit="FT">4</length></form>
    <weight>100</weight>
    <location unit="IN"><x>130</x><y>0</y><z>0</z></location>
  </pointmass>
</mass_balance>"#;

#[test]
fn weight_and_cg_include_point_masses() {
    let mut fdm = loaded(TWO_MASSES);
    assert_eq!(fdm.get_property("inertia/weight-lbs").unwrap(), 1300.0);
    run_mass_balance(&mut fdm);
    let cg = (1000.0 * 100.0 + 200.0 * 40.0 + 100.0 * 130.0) / 1300.0;
    let got = fdm.get_property("inertia/cg-x-in").unwrap();
    assert!((got - cg).abs() < 1e-12, "{got} vs {cg}");
    assert_eq!(
        fdm.get_property("inertia/mass-slugs").unwrap(),
        1300.0 * LBTOSLUG
    );
}

#[test]
fn jinv_inverts_j() {
    let mut fdm = loaded(TWO_MASSES);
    run_mass_balance(&mut fdm);
    let mb = &fdm.models.mass_balance;
    let product = mb.j() * mb.jinv();
    for r in Axis::ALL {
        for c in Axis::ALL {
            let expected = if r == c { 1.0 } else { 0.0 };
            assert!((product[(r, c)] - expected).abs() < 1e-12);
        }
    }
    // The cross product property keeps the tensor's sign, unlike ixy/iyz.
    let j13 = mb.j[(E_X, E_Z)];
    assert_eq!(fdm.get_property("inertia/ixz-slugs_ft2").unwrap(), j13);
}

#[test]
fn point_mass_properties_are_writable_and_weight_updates_shape_inertia() {
    let mut fdm = loaded(TWO_MASSES);
    assert!(
        fdm.set_property("inertia/pointmass-weight-lbs[1]", 200.0)
            .unwrap()
    );
    let tube = &fdm.models.mass_balance.point_masses()[1];
    assert_eq!(tube.inertia[(E_X, E_X)], (200.0 / SLUGTOLB) * 1.0 * 1.0);
    assert!(
        fdm.set_property("inertia/pointmass-location-X-inches[0]", 50.0)
            .unwrap()
    );
    assert_eq!(
        fdm.models.mass_balance.point_masses()[0].location[E_X],
        50.0
    );
    assert_eq!(
        fdm.get_property("inertia/pointmass-weight-lbs").unwrap(),
        200.0
    );
    assert!(!fdm.set_property("inertia/weight-lbs", 1.0).unwrap());
}

#[test]
fn delta_cg_tracks_moves_after_the_first_run() {
    let mut fdm = loaded(TWO_MASSES);
    run_mass_balance(&mut fdm);
    assert_eq!(
        fdm.models.mass_balance.delta_xyz_cg(),
        ColumnVector3::zero()
    );
    fdm.set_property("inertia/pointmass-location-X-inches[0]", 53.0)
        .unwrap();
    run_mass_balance(&mut fdm);
    let dx = fdm.models.mass_balance.delta_xyz_cg()[E_X];
    assert!((dx - 200.0 * 13.0 / 1300.0).abs() < 1e-12, "{dx}");
}

#[test]
fn hold_down_nudges_the_vehicle_by_the_cg_shift() {
    let mut fdm = loaded(TWO_MASSES);
    let identity = Matrix33::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
    fdm.models.propagate.frames.tb2i = identity;
    fdm.models.propagate.frames.tb2ec = identity;
    run_mass_balance(&mut fdm);
    fdm.models.exec.hold_down = true;
    let before = fdm.models.propagate.state.inertial_position;
    fdm.set_property("inertia/pointmass-location-X-inches[0]", 53.0)
        .unwrap();
    run_mass_balance(&mut fdm);
    // The tracked location is the CG: it moves aft (body -x) with the
    // point mass while the structure stays where it is.
    let shift = fdm.models.mass_balance.delta_xyz_cg()[E_X] * INCHTOFT;
    let moved = fdm.models.propagate.state.inertial_position[E_X] - before[E_X];
    assert!((moved + shift).abs() < 1e-12, "{moved} vs {shift}");
}

#[test]
fn point_mass_without_location_is_an_error() {
    let doc = parse_document(
        b"<mass_balance><pointmass name=\"p\"><weight>1</weight></pointmass></mass_balance>",
        "t.xml",
    )
    .unwrap();
    let mut fdm = Fdm::new().unwrap();
    assert!(matches!(
        fdm.load_mass_balance(&doc),
        Err(crate::Error::Xml(
            crate::xml::XmlError::MissingElement { .. }
        ))
    ));
}

#[test]
fn inertia_sign_convention_follows_the_attribute() {
    let xml = |attr: &str| {
        format!(
            "<mass_balance {attr}><ixx>1</ixx><iyy>1</iyy><izz>1</izz><ixy>2</ixy><ixz>3</ixz><iyz>4</iyz></mass_balance>"
        )
    };
    let negated = loaded(&xml(""));
    let plain = loaded(&xml("negated_crossproduct_inertia=\"false\""));
    assert_eq!(negated.models.mass_balance.base_j[(E_X, E_Y)], -2.0);
    assert_eq!(negated.models.mass_balance.base_j[(E_X, E_Z)], 3.0);
    assert_eq!(plain.models.mass_balance.base_j[(E_X, E_Y)], 2.0);
    assert_eq!(plain.models.mass_balance.base_j[(E_X, E_Z)], -3.0);
}
