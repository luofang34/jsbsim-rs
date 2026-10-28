// SPDX-License-Identifier: LGPL-2.1-or-later
use super::super::tests::{load_error, load_system};
use crate::{Error, LoadOptions};

fn system(body: &str) -> String {
    format!(
        "<system><property>test/a</property><channel name=\"c\"><angle name=\"g\"{body}</angle></channel></system>"
    )
}

#[test]
fn angles_need_both_angles_and_a_known_unit() {
    for (body, expected) in [
        (
            "><source_angle>test/a</source_angle>",
            "Target angle is required for Angles component: g",
        ),
        (
            "><target_angle>test/a</target_angle>",
            "Source angle is required for Angles component: g",
        ),
        (
            " unit=\"GRAD\"><source_angle>test/a</source_angle><target_angle>test/a</target_angle>",
            "Unknown unit GRAD in angle component, g",
        ),
    ] {
        let err = load_error(&system(body), LoadOptions::default());
        assert!(
            matches!(&err, Error::Model { message } if message.contains(expected)),
            "{body}: {err:?}"
        );
    }
}

#[test]
fn an_angle_missing_at_load_fails_the_run() {
    let body = "><source_angle>test/a</source_angle><target_angle>test/later</target_angle>";
    let mut fdm = load_system(&system(body), LoadOptions::default()).unwrap();
    // Created after the load, as JSBSim's GetNode never sees it.
    fdm.set_property("test/later", 1.0).unwrap();
    let err = fdm.run_ic().unwrap_err();
    assert!(err.to_string().contains("test/later"), "{err}");
}
