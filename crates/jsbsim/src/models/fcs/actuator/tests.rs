// SPDX-License-Identifier: LGPL-2.1-or-later
use super::super::tests::{load_error, load_system};
use crate::{Error, LoadOptions};

const LAGGED: &str = r#"<system><property>test/x</property><channel name="c">
  <actuator name="a">
    <input>test/x</input>
    <lag>10</lag>
    <rate_limit>1</rate_limit>
    <hysteresis_width>0.5</hysteresis_width>
  </actuator>
</channel></system>"#;

#[test]
fn a_trim_frame_passes_the_input_through_lag_rate_limit_and_hysteresis() {
    let mut fdm = load_system(LAGGED, LoadOptions::default()).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("test/x", 3.0).unwrap();
    fdm.run().unwrap();
    let held = fdm.get_property("fcs/a").unwrap();
    assert!(held < 3.0, "{held}");
    fdm.models.exec.trim_status = true;
    fdm.run().unwrap();
    assert_eq!(fdm.get_property("fcs/a").unwrap(), 3.0);
}

#[test]
fn malfunctions_are_boolean_properties() {
    let mut fdm = load_system(LAGGED, LoadOptions::default()).unwrap();
    fdm.set_property("fcs/a/malfunction/fail_zero", 0.25)
        .unwrap();
    assert_eq!(
        fdm.get_property("fcs/a/malfunction/fail_zero").unwrap(),
        1.0
    );
    let super::Kind::Actuator(a) = &fdm.models.fcs.channels[0].components[0].kind else {
        panic!("not an actuator");
    };
    assert!(a.fail_zero && !a.fail_stuck && !a.fail_hardover);
    assert_eq!(fdm.get_property("fcs/a/saturated").unwrap(), 0.0);
}

#[test]
fn an_actuator_needs_an_input() {
    let system = r#"<system><channel name="c"><actuator name="a"/></channel></system>"#;
    let err = load_error(system, LoadOptions::default());
    assert!(
        matches!(&err, Error::Model { message } if message.contains("Not enough <input> nodes")),
        "{err:?}"
    );
}

#[test]
fn a_hardover_below_zero_without_a_max_limit_fails_where_jsbsim_crashes() {
    let system = r#"<system><property>test/x</property><channel name="c">
  <actuator name="a">
    <input>test/x</input>
    <clipto><min>-1</min></clipto>
  </actuator>
</channel></system>"#;
    let mut fdm = load_system(system, LoadOptions::default()).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("fcs/a/malfunction/fail_hardover", 1.0)
        .unwrap();
    fdm.set_property("test/x", 2.0).unwrap();
    fdm.run().unwrap();
    // Without a usable <clipto> the upper limit is 0.
    assert_eq!(fdm.get_property("fcs/a").unwrap(), 0.0);
    fdm.set_property("test/x", -2.0).unwrap();
    let err = fdm.run().unwrap_err();
    assert!(err.to_string().contains("fail_hardover"), "{err}");
}
