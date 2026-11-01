// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use crate::{Error, Fdm, MemoryResolver};

#[test]
fn components_carry_jsbsim_names_and_units() {
    let expected = [
        (Component::Vt, "Vt", "ft/s"),
        (Component::Alpha, "Alpha", "rad"),
        (Component::Theta, "Theta", "rad"),
        (Component::Q, "Q", "rad/s"),
        (Component::Rpm0, "Rpm0", "rev/min"),
        (Component::Rpm1, "Rpm1", "rev/min"),
        (Component::Rpm2, "Rpm2", "rev/min"),
        (Component::Rpm3, "Rpm3", "rev/min"),
        (Component::Beta, "Beta", "rad"),
        (Component::Phi, "Phi", "rad"),
        (Component::P, "P", "rad/s"),
        (Component::Psi, "Psi", "rad"),
        (Component::R, "R", "rad/s"),
        (Component::Latitude, "Latitude", "rad"),
        (Component::Longitude, "Longitude", "rad"),
        (Component::Alt, "Alt", "ft"),
        (Component::ThrottleCmd, "ThtlCmd", "norm"),
        (Component::DaCmd, "DaCmd", "norm"),
        (Component::DeCmd, "DeCmd", "norm"),
        (Component::DrCmd, "DrCmd", "norm"),
    ];
    for (c, name, unit) in expected {
        assert_eq!((c.name(), c.unit()), (name, unit));
    }
}

/// A body without engines, which JSBSim's FGLinearization cannot handle.
#[test]
fn an_aircraft_without_engines_is_refused() {
    let mut r = MemoryResolver::new();
    r.insert(
        "aircraft/box/box.xml",
        r#"<fdm_config name="box">
  <metrics/>
  <mass_balance>
    <ixx>10</ixx><iyy>10</iyy><izz>10</izz><emptywt>100</emptywt>
    <location name="CG" unit="IN"><x>0</x><y>0</y><z>0</z></location>
  </mass_balance>
  <ground_reactions/>
</fdm_config>"#,
    );
    let mut fdm = Fdm::new().unwrap();
    fdm.set_property("simulation/models/FGAerodynamics/enabled", 0.0)
        .unwrap();
    fdm.load_model(&r, "aircraft", "box").unwrap();
    fdm.set_property("ic/h-sl-ft", 1000.0).unwrap();
    fdm.run_ic().unwrap();
    assert!(matches!(fdm.linearize(), Err(Error::Model { .. })));
    assert!(matches!(fdm.do_linearization(), Err(Error::Model { .. })));
    assert!(
        fdm.set_property("simulation/do_linearization", 1.0)
            .is_err()
    );
    assert!(fdm.last_linearization().is_none());
}

/// An engine that cannot run gets past the state list, so the failure comes
/// from a frame the linearization runs with integration suspended.
#[test]
fn a_failed_frame_gives_the_time_step_back_and_keeps_no_result() {
    let mut r = MemoryResolver::new();
    r.insert(
        "aircraft/t/t.xml",
        r#"<fdm_config name="t">
  <metrics/>
  <mass_balance>
    <ixx>100</ixx><iyy>200</iyy><izz>300</izz><emptywt>1000</emptywt>
    <location name="CG" unit="IN"><x>0</x><y>0</y><z>0</z></location>
  </mass_balance>
  <ground_reactions/>
  <propulsion>
    <tank type="FUEL">
      <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
      <capacity>100</capacity><contents>100</contents>
    </tank>
    <tank type="OXIDIZER">
      <location unit="IN"><x>0</x><y>0</y><z>0</z></location>
      <capacity>100</capacity><contents>100</contents>
    </tank>
    <engine>
      <feed>0</feed><feed>1</feed><feed>7</feed>
      <test_engine name="r0"><sl-fuel-flow>120</sl-fuel-flow></test_engine>
      <thruster><test_thruster name="t0"/></thruster>
    </engine>
  </propulsion>
  <aerodynamics/>
</fdm_config>"#,
    );
    let mut fdm = Fdm::new().unwrap();
    fdm.load_model(&r, "aircraft", "t").unwrap();
    fdm.set_property("ic/h-sl-ft", 10000.0).unwrap();
    let dt = fdm.get_property("simulation/dt").unwrap();
    assert_ne!(dt, 0.0);
    let got = fdm.do_linearization().map(|l| l.x.clone());
    assert!(matches!(got, Err(Error::Model { .. })), "{got:?}");
    assert_eq!(fdm.get_property("simulation/dt").unwrap(), dt);
    assert!(fdm.last_linearization().is_none());
}

#[test]
fn a_matrix_reads_by_row_then_column() {
    let m = Matrix::from_row_major(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    assert_eq!((m.nrows(), m.ncols(), m.shape()), (2, 3, (2, 3)));
    assert_eq!(m.as_slice(), [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    assert_eq!(m.row(1), Some(&[4.0, 5.0, 6.0][..]));
    assert_eq!(m.row(2), None);
    assert_eq!(m.rows().len(), 2);
    assert_eq!(
        m.rows().collect::<Vec<_>>(),
        [&[1.0, 2.0, 3.0][..], &[4.0, 5.0, 6.0][..]]
    );
    assert_eq!(m.get(1, 2), Some(6.0));
    // Past the end of a row is outside the matrix, not the next row.
    assert_eq!(m.get(0, 3), None);
    assert_eq!(m.get(2, 0), None);
    assert_eq!(m.get(usize::MAX, usize::MAX), None);
}

#[test]
fn a_matrix_without_columns_still_has_its_rows() {
    let m = Matrix::from_row_major(2, 0, Vec::new());
    assert_eq!(m.rows().len(), 2);
    assert_eq!(m.rows().count(), 2);
    assert_eq!(m.row(1), Some(&[][..]));
    assert_eq!(m.row(2), None);
    assert_eq!(m.get(0, 0), None);
}

#[test]
fn a_matrix_without_rows_has_nothing_to_read() {
    let m = Matrix::from_row_major(0, 3, Vec::new());
    assert_eq!(m.shape(), (0, 3));
    assert_eq!(m.rows().len(), 0);
    assert_eq!(m.row(0), None);
    assert_eq!(m.get(0, 0), None);
}
