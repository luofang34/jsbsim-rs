// SPDX-License-Identifier: LGPL-2.1-or-later
//! Files JSBSim's `Load` returns false for, or throws on.

use super::{fdm, load};
use crate::math::Axis;
use crate::{Error, MemoryResolver};

const ECEF_POSITION: &str = r#"<position frame="ECEF"><x unit="FT"> 21000000 </x></position>"#;

fn v2(body: &str) -> String {
    format!("<initialize version=\"2\">\n{body}\n</initialize>")
}

fn resolver(xml: &str) -> MemoryResolver {
    let mut resolver = MemoryResolver::new();
    resolver.insert("aircraft/x/init.xml", xml.as_bytes().to_vec());
    resolver
}

/// Every path on which `Load` returns false: `load_initial_conditions`
/// fails at the element responsible, where `Load` itself reports false.
#[test]
fn each_inconsistency_fails_at_its_element() {
    let cases = [
        (r#"<initialize version="0.5"/>"#.to_owned(), 1, "below 1"),
        (
            "<initialize>\n<latitude unit=\"DEG\"> 95 </latitude>\n</initialize>".to_owned(),
            2,
            "The latitude value 95 DEG",
        ),
        (v2(""), 1, "Initial position not specified"),
        (
            v2(r#"<position frame="ECEF"><longitude unit="DEG"> 3 </longitude></position>"#),
            2,
            "No altitude or radius",
        ),
        (
            v2(
                r#"<position frame="ECEF"><altitudeMSL unit="FT"> 1 </altitudeMSL>
                <latitude unit="DEG"> 95 </latitude></position>"#,
            ),
            3,
            "The latitude value 95 DEG",
        ),
        (
            v2(r#"<position frame="HOME"/>"#),
            2,
            "Neither ECI nor ECEF frame",
        ),
        (
            v2(&format!("{ECEF_POSITION}\n<orientation frame=\"HOME\"/>")),
            3,
            "Orientation frame type: \"home\"",
        ),
        (
            v2(&format!("{ECEF_POSITION}\n<velocity frame=\"HOME\"/>")),
            3,
            "Velocity frame type: \"home\"",
        ),
        (
            v2(&format!("{ECEF_POSITION}\n<attitude_rate frame=\"HOME\"/>")),
            3,
            "Attitude rate frame type: \"home\"",
        ),
    ];
    for (xml, line, expected) in cases {
        let e = load(&mut fdm(), &xml).unwrap_err();
        assert!(matches!(&e, Error::Model { .. }), "{xml}: {e:?}");
        assert!(e.to_string().contains(expected), "{xml}: {e}");
        let at = e.location().unwrap();
        assert_eq!(at.line, line, "{xml}: {e}");
        assert!(at.file.ends_with("init.xml"), "{xml}: {e}");
        assert_eq!(
            fdm().load_initial_conditions_raw(&resolver(&xml), "aircraft/x", "init"),
            Ok(false),
            "{xml}"
        );
    }
}

/// What JSBSim throws on stays apart from what it returns false for.
#[test]
fn a_file_jsbsim_throws_on_is_an_error_to_the_raw_load_too() {
    for xml in [r#"<initialize version="3"/>"#, "<reset/>"] {
        let raw = fdm().load_initial_conditions_raw(&resolver(xml), "aircraft/x", "init");
        assert!(matches!(raw, Err(Error::Model { .. })), "{xml}: {raw:?}");
        assert!(load(&mut fdm(), xml).unwrap_err().location().is_some());
    }
}

/// A document that is not `<initialize>` names its file once, in the
/// location, and its root element in the message.
#[test]
fn a_file_that_is_not_a_reset_file_says_what_it_is() {
    let e = load(&mut fdm(), "<reset/>").unwrap_err();
    let text = e.to_string();
    assert!(
        text.ends_with("not a reset file: its root element is <reset>, not <initialize>"),
        "{text}"
    );
    assert_eq!(text.matches("init.xml").count(), 1, "{text}");
}

#[test]
fn the_first_inconsistency_is_the_one_reported() {
    let xml = v2(r#"<position frame="HOME"/>
<orientation frame="HOME"/>
<velocity frame="HOME"/>"#);
    let (logs, result) = crate::test_log::logged(|| load(&mut fdm(), &xml));
    let e = result.unwrap_err();
    assert_eq!(e.location().unwrap().line, 2, "{e}");
    // The returned error is not logged too; the two it does not carry are.
    let errors: Vec<_> = logs
        .iter()
        .filter(|(level, _)| *level == tracing::Level::ERROR)
        .map(|(_, m)| m.as_str())
        .collect();
    assert_eq!(errors.len(), 2, "{logs:?}");
    assert!(errors.iter().all(|m| !m.contains("line 2")), "{logs:?}");
}

/// JSBSim's `Load` returns false for a version below 1 without a word; the
/// error says why, and nothing is logged.
#[test]
fn a_version_below_1_fails_without_logging() {
    let (logs, result) =
        crate::test_log::logged(|| load(&mut fdm(), r#"<initialize version="0.5"/>"#));
    assert!(result.unwrap_err().to_string().contains("below 1"));
    assert!(
        logs.iter().all(|(level, _)| *level > tracing::Level::WARN),
        "{logs:?}"
    );
}

/// A version 2 load goes on past each frame it cannot use, then reads
/// `<running>`, and a changed planet rotation reaches the models, as
/// JSBSim's mutations stand when `Load` returns false.
#[test]
fn the_rest_of_a_version_2_file_applies_past_an_inconsistency() {
    let mut fdm = fdm();
    let xml = v2(
        r#"<planet_rotation_rate unit="RAD"> 0.001 </planet_rotation_rate>
<position frame="HOME"/>
<orientation frame="HOME"/>
<velocity unit="FT/SEC" frame="LOCAL"><x> 7 </x></velocity>
<attitude_rate unit="RAD/SEC" frame="BODY"><x> 0.25 </x></attitude_rate>
<running> 1 </running>"#,
    );
    let e = load(&mut fdm, &xml).unwrap_err();
    assert_eq!(e.location().unwrap().line, 3, "{e}");
    let ic = &fdm.models.ic;
    assert_eq!(ic.ned_vel_fps(Axis::X), 7.0);
    assert_eq!(ic.pqr_body[Axis::X], 0.25);
    assert_eq!(ic.engines_running, 0b10);
    assert_eq!(fdm.models.propagate.input.omega_planet[Axis::Z], 0.001);
    assert_eq!(fdm.models.accelerations.input.omega_planet[Axis::Z], 0.001);
}

#[test]
fn the_rest_of_a_version_1_file_applies_past_a_bad_latitude() {
    let mut fdm = fdm();
    let xml = r#"<initialize>
<latitude unit="DEG"> 95 </latitude>
<ubody unit="FT/SEC"> 9 </ubody>
<running> 0 </running>
</initialize>"#;
    load(&mut fdm, xml).unwrap_err();
    assert_eq!(fdm.get_property("ic/u-fps").unwrap(), 9.0);
    assert_eq!(fdm.models.ic.engines_running, 1);
}

#[test]
fn a_version_below_1_still_reads_running() {
    let mut fdm = fdm();
    load(
        &mut fdm,
        r#"<initialize version="0.5"><running> -1 </running></initialize>"#,
    )
    .unwrap_err();
    assert_eq!(fdm.models.ic.engines_running, u32::MAX);
}

#[test]
fn load_initial_conditions_file_fails_alike() {
    let mut files = MemoryResolver::new();
    files.insert("reset.xml", r#"<initialize version="2"/>"#);
    let e = fdm()
        .load_initial_conditions_file(&files, "reset")
        .unwrap_err();
    assert_eq!(e.location().unwrap().line, 1, "{e}");
}
