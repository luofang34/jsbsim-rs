// SPDX-License-Identifier: LGPL-2.1-or-later
use super::super::tests::{load_error, load_system};
use crate::{Error, LoadOptions};

#[test]
fn a_linear_actuator_needs_an_input() {
    let system = r#"<system><channel name="c"><linear_actuator name="l"/></channel></system>"#;
    let err = load_error(system, LoadOptions::default());
    assert!(
        matches!(&err, Error::Model { message } if message.contains("Not enough <input> nodes")),
        "{err:?}"
    );
}

#[test]
fn out_of_range_parameters_are_forced_to_jsbsims_values() {
    let system = r#"<system><property>test/x</property><channel name="c">
  <linear_actuator name="l">
    <input>test/x</input>
    <module>-4</module>
    <hysteresis>-1</hysteresis>
    <lag>-2</lag>
    <rate>0</rate>
    <versus>-1.9</versus>
  </linear_actuator>
</channel></system>"#;
    let fdm = load_system(system, LoadOptions::default()).unwrap();
    let kind = &fdm.models.fcs.channels[0].components[0].kind;
    let crate::models::fcs::Kind::LinearActuator(l) = kind else {
        panic!("{kind:?}");
    };
    assert_eq!(
        (l.module, l.hysteresis, l.lag, l.rate, l.versus),
        (1.0, 0.0, 0.0, 0.5, -1)
    );
}
