// SPDX-License-Identifier: LGPL-2.1-or-later

use jsbsim::oracle_support::xml::parse_document;

use super::super::daveml::Model;
use super::{TOLERANCE, compare_series, properties, report};
use crate::Series;

/// el = 2 alpha + latOffset, with latOffset an input as in F16_control.dml.
const MODEL: &str = r#"<DAVEfunc>
  <variableDef name="alpha" varID="alpha" units="deg"/>
  <variableDef name="offset" varID="latOffset" units="ft"/>
  <variableDef name="el" varID="el" units="deg">
    <calculation><math><apply><plus/>
      <apply><times/><cn>2</cn><ci>alpha</ci></apply><ci>latOffset</ci>
    </apply></math></calculation>
  </variableDef>
</DAVEfunc>"#;

fn model() -> Model {
    Model::from_element("m", &parse_document(MODEL.as_bytes(), "m").unwrap()).unwrap()
}

fn series(rows: &[[f64; 5]]) -> Series {
    Series {
        columns: [
            "Time",
            "f16/air/alpha-deg",
            "f16/gnc/lat-offset-ft",
            "f16/control/el-deg",
            "f16/gnc/guidance-on",
        ]
        .map(str::to_owned)
        .to_vec(),
        rows: rows.iter().map(|r| r.to_vec()).collect(),
    }
}

#[test]
fn each_frame_is_evaluated_on_its_own_inputs() {
    let s = series(&[[0.0, 1.0, 0.5, 2.5, 0.0], [0.0025, 2.0, -1.0, 3.0, 0.0]]);
    let m = model();
    let worst = compare_series((&m, &m), &s, false).unwrap();
    assert_eq!(worst["el"].rel, 0.0);
    assert_eq!(report("test", &worst), 0);
}

#[test]
fn a_frame_off_by_more_than_the_tolerance_fails_at_its_time() {
    let off = 3.0 * (1.0 + 10.0 * TOLERANCE);
    let s = series(&[[0.0, 1.0, 0.5, 2.5, 0.0], [0.0025, 2.0, -1.0, off, 0.0]]);
    let m = model();
    let worst = compare_series((&m, &m), &s, false).unwrap();
    assert_eq!(worst["el"].time, 0.0025);
    assert_eq!(report("test", &worst), 1);
}

#[test]
fn under_guidance_the_course_inputs_are_not_given() {
    // F16_gnc.dml computes latOffset, so this model lacks an input on the
    // guided row.
    let m = model();
    let s = series(&[[0.0, 1.0, 0.5, 2.5, 0.0], [0.0025, 1.0, 0.5, 2.5, 1.0]]);
    assert!(compare_series((&m, &m), &s, false).is_err());
    let s = series(&[[0.0, 1.0, 0.5, 2.5, 0.0]]);
    assert!(compare_series((&m, &m), &s, false).is_ok());
}

#[test]
fn every_property_is_sampled_once() {
    let p = properties();
    let mut unique = p.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), p.len());
    assert!(p.contains(&"f16/gnc/guidance-on") && p.contains(&"f16/control/pwr-pct"));
}
