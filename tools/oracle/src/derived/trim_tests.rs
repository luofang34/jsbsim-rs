// SPDX-License-Identifier: LGPL-2.1-or-later
//! The noise-free c172x variants of JSBSim's trim tests, each with
//! `scripts/c1722.xml` renamed onto it:
//!
//! - [`ACCELEROMETERS`]: with `<system file="accelerometers"/>` appended,
//!   as `TestAccelerometer.testSteadyFlight` derives it;
//! - [`ELEVATOR_DELAY`]: with a 0.1 s `<delay>` on its elevator actuator,
//!   as `CheckTrim.test_trim_with_actuator_delay` derives it;
//!
//! and `tests/tripod.xml` unchanged as `aircraft/tripod/tripod.xml`, where
//! `LoadModel` finds it as `TestGndReactions` loads it.

use super::piston::{Read, fail, quiet_autopilot, renamed_script, with_header};
use crate::OracleError;

/// The c172x with accelerometers.
pub const ACCELEROMETERS: &str = "c172x_accelerometers";

/// The c172x with a delayed elevator actuator.
pub const ELEVATOR_DELAY: &str = "c172x_elevator_delay";

const SOURCE: &str = "aircraft/c172x/c172x.xml";

/// The [`ACCELEROMETERS`] aircraft.
pub fn accelerometers_aircraft(text: &str) -> Result<String, OracleError> {
    let close = text
        .rfind("</fdm_config>")
        .ok_or_else(|| fail(SOURCE, "no </fdm_config>"))?;
    let mut body = text.to_owned();
    body.insert_str(close, "  <system file=\"accelerometers\"/>\n");
    with_header(
        &body,
        "fdm_config",
        &format!(
            "{ACCELEROMETERS}: {SOURCE} with <system file=\"accelerometers\"/>\n  appended, as JSBSim's tests/TestAccelerometer.py derives it; its\n  autopilot c172ap.xml has its sensor noise set to 0."
        ),
        SOURCE,
    )
}

/// The [`ELEVATOR_DELAY`] aircraft.
pub fn elevator_delay_aircraft(text: &str) -> Result<String, OracleError> {
    let start = text
        .find("<actuator name=\"fcs/elevator-actuator\">")
        .ok_or_else(|| fail(SOURCE, "no elevator actuator"))?;
    let close = text[start..]
        .find("</actuator>")
        .ok_or_else(|| fail(SOURCE, "unterminated elevator actuator"))?
        + start;
    let mut body = text.to_owned();
    body.insert_str(close, "  <delay>0.1</delay>\n            ");
    with_header(
        &body,
        "fdm_config",
        &format!(
            "{ELEVATOR_DELAY}: {SOURCE} with a 0.1 s <delay> on its elevator\n  actuator, as JSBSim's tests/CheckTrim.py derives it; its autopilot\n  c172ap.xml has its sensor noise set to 0."
        ),
        SOURCE,
    )
}

/// Both aircraft, their autopilot and initial conditions, and c1722 on
/// each.
pub fn files(read: Read<'_>, files: &mut Vec<(String, String)>) -> Result<(), OracleError> {
    files.push((
        "aircraft/tripod/tripod.xml".to_owned(),
        read("tests/tripod.xml")?,
    ));
    let c172x = read(SOURCE)?;
    let ap = "aircraft/c172x/c172ap.xml";
    let c1722 = read("scripts/c1722.xml")?;
    for (name, aircraft) in [
        (ACCELEROMETERS, accelerometers_aircraft(&c172x)?),
        (ELEVATOR_DELAY, elevator_delay_aircraft(&c172x)?),
    ] {
        let dir = format!("aircraft/{name}");
        files.push((format!("{dir}/{name}.xml"), aircraft));
        files.push((
            format!("{dir}/c172ap.xml"),
            quiet_autopilot(&read(ap)?, ap)?,
        ));
        files.push((
            format!("{dir}/reset01.xml"),
            read("aircraft/c172x/reset01.xml")?,
        ));
        let suffix = name.trim_start_matches("c172x_");
        files.push((
            format!("scripts/c1722_{suffix}.xml"),
            renamed_script(&c1722, "c1722", "c172x", name)?,
        ));
    }
    Ok(())
}
