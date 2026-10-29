// SPDX-License-Identifier: LGPL-2.1-or-later
//! Scenarios for what the upstream scripts and tests leave out of the
//! piston engine and the propeller, flown by copies of upstream aircraft:
//!
//! - `piston_start_c310.xml`: the c310 on the runway cranked by its
//!   starter without spark, started by its magnetos, run on each magneto
//!   alone, leaned, cut off by its mixture and restarted;
//! - `piston_governor_c310.xml`: the c310 started running in flight, its
//!   constant-speed governors moved through their range, one propeller
//!   feathered and unfeathered, and the other engine's magnetos cut so
//!   that its propeller windmills, then restored so that it restarts;
//! - `piston_boost_boeing314.xml`: the Boeing 314 started running in
//!   cruise from [`CRUISE_IC`], its supercharged engines at full throttle
//!   (clamped at the takeoff boost), then below 98% (at the rated boost)
//!   and at half throttle.

use super::piston::Read;
use crate::OracleError;

/// The c310 and Boeing 314 files the scenarios fly, by their upstream
/// path.
pub const COPIES: [&str; 8] = [
    "aircraft/c310/c310.xml",
    "aircraft/c310/c310ap.xml",
    "aircraft/c310/reset00.xml",
    "engine/engIO470D.xml",
    "engine/propC10v.xml",
    "aircraft/Boeing314/Boeing314.xml",
    "engine/WrightGR-2600.xml",
    "engine/HamiltonStd6243A-3.xml",
];

/// `aircraft/Boeing314/cruise.xml`, which upstream has no initial
/// condition: level at 8000 ft and 150 kt.
pub const CRUISE_IC: &str = r#"<?xml version="1.0"?>
<!--
  SPDX-License-Identifier: LGPL-2.1-or-later
  cruise.xml: the Boeing 314 level at 8000 ft and 150 kt, for the piston
  engine scenarios. Regenerate with `oracle derive`.
-->
<initialize name="cruise">
  <latitude unit="DEG"> 37.6 </latitude>
  <longitude unit="DEG"> -122.4 </longitude>
  <altitude unit="FT"> 8000.0 </altitude>
  <vc unit="KTS"> 150.0 </vc>
  <theta unit="DEG"> 2.0 </theta>
  <psi unit="DEG"> 270.0 </psi>
</initialize>
"#;

fn header(name: &str, what: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?>\n<!--\n  SPDX-License-Identifier: LGPL-2.1-or-later\n  {name}: {what}\n  Regenerate with `oracle derive`.\n-->\n"
    )
}

/// An event at `t` s setting each `(property, value)`.
fn event(name: &str, t: f64, sets: &[(&str, f64)]) -> String {
    let mut out = format!(
        "    <event name=\"{name}\">\n      <condition> simulation/sim-time-sec ge {t} </condition>\n"
    );
    for (property, value) in sets {
        out.push_str(&format!(
            "      <set name=\"{property}\" value=\"{value}\"/>\n"
        ));
    }
    out.push_str("    </event>\n");
    out
}

fn both(property: &str, value: f64) -> [(String, f64); 2] {
    [
        (format!("{property}[0]"), value),
        (format!("{property}[1]"), value),
    ]
}

fn script(name: &str, what: &str, aircraft: (&str, &str), end: f64, body: &str) -> String {
    let (aircraft, ic) = aircraft;
    format!(
        "{}<runscript name=\"{name}\">\n  <use aircraft=\"{aircraft}\" initialize=\"{ic}\"/>\n  <run start=\"0.0\" end=\"{end}\" dt=\"0.008333333333333333\">\n{body}  </run>\n</runscript>\n",
        header(&format!("{name}.xml"), what)
    )
}

fn owned(sets: &[(String, f64)]) -> Vec<(&str, f64)> {
    sets.iter().map(|(p, v)| (p.as_str(), *v)).collect()
}

/// `piston_start_c310.xml`.
#[must_use]
pub fn start_script() -> String {
    let mut crank: Vec<(String, f64)> = Vec::new();
    crank.extend(both("fcs/throttle-cmd-norm", 0.3));
    crank.extend(both("fcs/mixture-cmd-norm", 1.0));
    crank.push(("propulsion/starter_cmd".to_owned(), 1.0));
    let lean = both("fcs/mixture-cmd-norm", 0.6);
    let mut body = String::from(
        "    <property value=\"1\"> fcs/left-brake-cmd-norm </property>\n    <property value=\"1\"> fcs/right-brake-cmd-norm </property>\n",
    );
    body.push_str(&event("crank without spark", 0.5, &owned(&crank)));
    body.push_str(&event(
        "magnetos on",
        2.0,
        &[("propulsion/magneto_cmd", 3.0)],
    ));
    body.push_str(&event(
        "starter off",
        5.0,
        &[("propulsion/starter_cmd", 0.0)],
    ));
    body.push_str(&event(
        "left magneto",
        8.0,
        &[("propulsion/magneto_cmd", 1.0)],
    ));
    body.push_str(&event(
        "right magneto",
        10.0,
        &[("propulsion/magneto_cmd", 2.0)],
    ));
    body.push_str(&event(
        "both magnetos",
        12.0,
        &[("propulsion/magneto_cmd", 3.0)],
    ));
    body.push_str(&event("lean", 14.0, &owned(&lean)));
    body.push_str(&event("cut off", 17.0, &[("fcs/mixture-cmd-norm[0]", 0.0)]));
    body.push_str(&event(
        "restart",
        22.0,
        &[
            ("fcs/mixture-cmd-norm[0]", 1.0),
            ("propulsion/starter_cmd", 1.0),
        ],
    ));
    body.push_str(&event(
        "restarted",
        25.0,
        &[("propulsion/starter_cmd", 0.0)],
    ));
    script(
        "piston_start_c310",
        "the c310 on the runway cranked\n  without spark, started by its magnetos, run on each magneto alone,\n  leaned, cut off by its mixture and restarted.",
        ("c310", "reset00"),
        30.0,
        &body,
    )
}

/// `piston_governor_c310.xml`.
#[must_use]
pub fn governor_script() -> String {
    let mut start: Vec<(String, f64)> = Vec::new();
    start.extend(both("fcs/throttle-cmd-norm", 1.0));
    start.extend(both("fcs/mixture-cmd-norm", 1.0));
    start.extend(both("fcs/advance-cmd-norm", 1.0));
    start.push(("propulsion/set-running".to_owned(), -1.0));
    let mut body = String::new();
    for (p, v) in [
        ("ic/h-sl-ft", 6000.0),
        ("ic/u-fps", 280.0),
        ("ic/theta-deg", 2.0),
    ] {
        body.push_str(&format!("    <property value=\"{v}\"> {p} </property>\n"));
    }
    body.push_str(&event("start running", 0.05, &owned(&start)));
    body.push_str(&event(
        "governor half",
        5.0,
        &[("fcs/advance-cmd-norm[0]", 0.5)],
    ));
    body.push_str(&event(
        "governor low",
        10.0,
        &[("fcs/advance-cmd-norm[0]", 0.0)],
    ));
    body.push_str(&event("feather", 15.0, &[("fcs/feather-cmd-norm[1]", 1.0)]));
    body.push_str(&event(
        "magnetos off",
        25.0,
        &[
            ("propulsion/active_engine", 0.0),
            ("propulsion/magneto_cmd", 0.0),
        ],
    ));
    body.push_str(&event(
        "windmill restart",
        27.0,
        &[
            ("propulsion/magneto_cmd", 3.0),
            ("propulsion/active_engine", -1.0),
        ],
    ));
    body.push_str(&event(
        "unfeather",
        40.0,
        &[("fcs/feather-cmd-norm[1]", 0.0)],
    ));
    script(
        "piston_governor_c310",
        "the c310 started running in\n  flight, its governors moved through their range, one propeller feathered\n  and unfeathered, the other engine's magnetos cut to windmill and restored.",
        ("c310", "reset00"),
        50.0,
        &body,
    )
}

/// `piston_boost_boeing314.xml`.
#[must_use]
pub fn boost_script() -> String {
    let throttle = |v: f64| -> Vec<(String, f64)> {
        (0..4)
            .map(|i| (format!("fcs/throttle-cmd-norm[{i}]"), v))
            .collect()
    };
    let mut start = throttle(1.0);
    for i in 0..4 {
        start.push((format!("fcs/mixture-cmd-norm[{i}]"), 1.0));
        start.push((format!("fcs/advance-cmd-norm[{i}]"), 1.0));
    }
    start.push(("propulsion/set-running".to_owned(), -1.0));
    let mut body = String::new();
    body.push_str(&event("start running", 0.05, &owned(&start)));
    body.push_str(&event("rated boost", 8.0, &owned(&throttle(0.9))));
    body.push_str(&event("half throttle", 14.0, &owned(&throttle(0.5))));
    script(
        "piston_boost_boeing314",
        "the Boeing 314 started running in\n  cruise with its supercharged engines at full throttle (the takeoff\n  boost), then at the rated boost and at half throttle.",
        ("Boeing314", "cruise"),
        20.0,
        &body,
    )
}

/// Every file of the scenarios, by its path under `oracle/derived`.
pub fn files(read: Read<'_>) -> Result<Vec<(String, String)>, OracleError> {
    let mut files = Vec::new();
    for rel in COPIES {
        files.push((rel.to_owned(), read(rel)?));
    }
    files.push(("scripts/piston_start_c310.xml".to_owned(), start_script()));
    files.push((
        "scripts/piston_governor_c310.xml".to_owned(),
        governor_script(),
    ));
    files.push((
        "aircraft/Boeing314/cruise.xml".to_owned(),
        CRUISE_IC.to_owned(),
    ));
    files.push((
        "scripts/piston_boost_boeing314.xml".to_owned(),
        boost_script(),
    ));
    Ok(files)
}
