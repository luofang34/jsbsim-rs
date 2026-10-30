// SPDX-License-Identifier: LGPL-2.1-or-later
use super::super::tests::{load_error, load_system};
use crate::{Error, LoadOptions};

const COORDINATES: &str = r"
      <target_latitude>  test/tgt-lat </target_latitude>
      <target_longitude> test/tgt-lon </target_longitude>
      <source_latitude>  test/src-lat </source_latitude>
      <source_longitude> test/src-lon </source_longitude>";

fn system(planet: &str, component: &str) -> String {
    format!(
        r#"{planet}<system>
  <property>test/tgt-lat</property>
  <property>test/tgt-lon</property>
  <property>test/src-lat</property>
  <property>test/src-lon</property>
  <channel name="c">
    {component}
  </channel>
</system>"#
    )
}

fn distance(unit: &str) -> String {
    format!(r#"<waypoint_distance name="test/d" {unit}>{COORDINATES}</waypoint_distance>"#)
}

fn model_message(err: &Error) -> &str {
    match err {
        Error::Model { message } => message,
        _ => "",
    }
}

#[test]
fn every_coordinate_is_required() {
    for (tag, what) in [
        ("target_latitude", "Target latitude"),
        ("target_longitude", "Target longitude"),
        ("source_latitude", "Source latitude"),
        ("source_longitude", "Source longitude"),
    ] {
        let line = COORDINATES
            .lines()
            .find(|l| l.contains(tag))
            .unwrap_or_default();
        let component = distance("").replace(line, "");
        let err = load_error(&system("", &component), LoadOptions::default());
        let expected = format!("{what} is required for waypoint component: test/d");
        assert!(model_message(&err).contains(&expected), "{err:?}");
    }
}

#[test]
fn an_unknown_output_unit_fails_to_load() {
    let err = load_error(
        &system("", &distance(r#"unit="KM""#)),
        LoadOptions::default(),
    );
    assert!(
        model_message(&err).contains("Unknown unit KM in DISTANCE waypoint component, test/d"),
        "{err:?}"
    );
    let heading =
        format!(r#"<waypoint_heading name="test/h" unit="FT">{COORDINATES}</waypoint_heading>"#);
    let err = load_error(&system("", &heading), LoadOptions::default());
    assert!(
        model_message(&err).contains("Unknown unit FT in HEADING waypoint component"),
        "{err:?}"
    );
}

/// The distance from (0, 0) to (0, 1 rad) on the planet `planet` declares.
fn equator_distance(planet: &str) -> f64 {
    let mut fdm = load_system(&system(planet, &distance("")), LoadOptions::default()).unwrap();
    fdm.set_property("test/tgt-lon", 1.0).unwrap();
    fdm.run_ic().unwrap();
    fdm.get_property("test/d").unwrap()
}

#[test]
fn the_distance_is_on_the_planet_ellipse_at_load() {
    // Along the equator the geodesic is the equator itself.
    let wgs84 = equator_distance("");
    assert!((wgs84 - 20_925_646.325_46).abs() < 1e-6, "{wgs84}");
    let sphere = r#"<planet>
    <semimajor_axis unit="FT"> 1000000 </semimajor_axis>
    <semiminor_axis unit="FT"> 1000000 </semiminor_axis>
  </planet>"#;
    let small = equator_distance(sphere);
    assert!((small - 1.0e6).abs() < 1e-6, "{small}");
}

#[test]
fn a_latitude_beyond_90_degrees_fails_the_run() {
    let mut fdm = load_system(&system("", &distance("")), LoadOptions::default()).unwrap();
    fdm.set_property("test/tgt-lat", 1.6).unwrap();
    let err = fdm.run_ic().unwrap_err();
    assert!(
        model_message(&err).contains(
            "Target latitude in waypoint \"test/d\" must be less than or equal to 90 degrees."
        ),
        "{err:?}"
    );
    fdm.set_property("test/tgt-lat", 0.0).unwrap();
    fdm.set_property("test/src-lat", -1.6).unwrap();
    let err = fdm.run().unwrap_err();
    assert!(
        model_message(&err).contains("Source latitude in waypoint \"test/d\""),
        "{err:?}"
    );
}
