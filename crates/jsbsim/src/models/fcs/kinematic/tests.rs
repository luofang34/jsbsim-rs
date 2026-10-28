// SPDX-License-Identifier: LGPL-2.1-or-later
use super::super::tests::{load_error, load_system};
use crate::{Error, LoadOptions};

const FLAPS: &str = r#"<system><property>test/x</property><channel name="c">
  <kinematic name="k">
    <input>test/x</input>
    <traverse>
      <setting> <position>0</position> <time>0</time> </setting>
      <setting> <position>10</position> <time>2</time> </setting>
    </traverse>
  </kinematic>
</channel></system>"#;

#[test]
fn a_trim_frame_reaches_the_input_at_once() {
    let mut fdm = load_system(FLAPS, LoadOptions::default()).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("test/x", 1.0).unwrap();
    fdm.run().unwrap();
    assert!(fdm.get_property("fcs/k").unwrap() < 10.0);
    fdm.models.exec.trim_status = true;
    fdm.run().unwrap();
    assert_eq!(fdm.get_property("fcs/k").unwrap(), 10.0);
}

#[test]
fn an_output_beyond_the_last_detent_moves_back_through_the_last_interval() {
    let mut fdm = load_system(FLAPS, LoadOptions::default()).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("test/x", 1.0).unwrap();
    fdm.set_property("fcs/k", 12.0).unwrap();
    fdm.run().unwrap();
    // 10 / 2 s over one 1/120 s step.
    let expected = 12.0 - 5.0 / 120.0;
    assert!((fdm.get_property("fcs/k").unwrap() - expected).abs() < 1e-12);
}

#[test]
fn a_kinematic_needs_a_traverse_of_two_settings() {
    for (body, expected) in [
        ("<input>x</input>", "has no <traverse>"),
        (
            "<input>x</input><traverse><setting><position>0</position><time>0</time></setting></traverse>",
            "must have more than 1 setting element",
        ),
    ] {
        let system = format!(
            "<system><channel name=\"c\"><kinematic name=\"k\">{body}</kinematic></channel></system>"
        );
        let err = load_error(&system, LoadOptions::default());
        assert!(
            matches!(&err, Error::Model { message } if message.contains(expected)),
            "{err:?}"
        );
    }
}
