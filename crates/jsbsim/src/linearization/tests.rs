// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use crate::MemoryResolver;

#[test]
fn components_carry_jsbsim_names_and_units() {
    let expected = [
        (Component::Vt, "Vt", "ft/s"),
        (Component::Alpha, "Alpha", "rad"),
        (Component::Theta, "Theta", "rad"),
        (Component::Q, "Q", "rad/s"),
        (Component::Rpm(0), "Rpm0", "rev/min"),
        (Component::Rpm(3), "Rpm3", "rev/min"),
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
    assert!(
        fdm.set_property("simulation/do_linearization", 1.0)
            .is_err()
    );
    assert!(fdm.last_linearization().is_none());
}
