// SPDX-License-Identifier: LGPL-2.1-or-later

use std::collections::BTreeMap;

use super::{LB_PER_SLUG, PROPERTIES, Value, report};

/// F16_inertia.dml's outputs at 25% MAC.
fn dml() -> BTreeMap<String, f64> {
    [
        ("XIXX", 9496.0),
        ("XIYY", 55814.0),
        ("XIZZ", 63100.0),
        ("XIZX", 982.0),
        ("XIXY", 0.0),
        ("XIYZ", 0.0),
        ("XMASS", 637.1595),
        ("DXCG", 1.132),
        ("DYCG", 0.0),
        ("DZCG", 0.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
}

/// JSBSim's properties of f16.xml loaded with the TM's sign of Ixz.
fn jsbsim(ixz: f64) -> Value {
    let values = [
        9496.0,
        55814.0,
        63100.0,
        0.0,
        ixz,
        0.0,
        637.1595,
        637.1595 * LB_PER_SLUG,
        -1.132 * 12.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ];
    Value(PROPERTIES.into_iter().zip(values).collect())
}

#[test]
fn the_tensor_carries_minus_ixz() {
    assert_eq!(report("test", &jsbsim(-982.0), &dml()), 0);
    // negated_crossproduct_inertia="true" would read <ixz> 982 as -Ixz.
    assert_eq!(report("test", &jsbsim(982.0), &dml()), 1);
}

#[test]
fn a_missing_value_fails_its_comparison() {
    let mut d = dml();
    d.remove("DXCG");
    assert_eq!(report("test", &jsbsim(-982.0), &d), 1);
}
