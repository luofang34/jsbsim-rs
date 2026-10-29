// SPDX-License-Identifier: LGPL-2.1-or-later
//! The c172x of the tests that start its engine from its initial
//! conditions:
//!
//! - [`INERTIA`]: the noise-free c172x with the `<inertia_factor>` that
//!   `TestFuelTanksInertia` adds to its first tank, and `scripts/c1722.xml`
//!   on it without its trim (FGTrim is not ported yet);
//! - `unusable_fuel.xml`: `TestUnusableFuel`'s settings as a script.

use super::piston::{NOISE0, Read, fail, quiet_autopilot, replace_once, with_header};
use crate::OracleError;

/// The noise-free c172x with an inertia factor on its first tank.
pub const INERTIA: &str = "c172x_inertia";

/// The [`INERTIA`] aircraft: the c172x with `<inertia_factor>` 1.0
/// appended to its first tank, as `test_fuel_tanks_inertia` edits it.
pub fn inertia_aircraft(text: &str) -> Result<String, OracleError> {
    let source = "aircraft/c172x/c172x.xml";
    let tank = text
        .find("<tank")
        .ok_or_else(|| fail(source, "no <tank>"))?;
    let close = text[tank..]
        .find("</tank>")
        .ok_or_else(|| fail(source, "no </tank>"))?
        + tank;
    let mut body = text.to_owned();
    body.insert_str(close, "<inertia_factor>1.0</inertia_factor>");
    with_header(
        &body,
        "fdm_config",
        &format!(
            "{INERTIA}: {source} with an <inertia_factor> of 1.0 appended to\n  its first tank, as JSBSim's tests/TestFuelTanksInertia.py edits it; its\n  autopilot c172ap.xml has its sensor noise set to 0."
        ),
        source,
    )
}

/// `c1722_untrimmed.xml`: `scripts/c1722.xml` on [`INERTIA`] without its
/// `Trim` event, the 200 s consumption run of `test_fuel_tanks_inertia`
/// short of FGTrim.
pub fn untrimmed_c1722(text: &str) -> Result<String, OracleError> {
    let source = "scripts/c1722.xml";
    let start = text
        .find("    <event name=\"Trim\">")
        .ok_or_else(|| fail(source, "no Trim event"))?;
    let end = text[start..]
        .find("</event>\n")
        .ok_or_else(|| fail(source, "unterminated Trim event"))?
        + start
        + "</event>\n".len();
    let mut body = text.to_owned();
    body.replace_range(start..end, "");
    let body = replace_once(
        &body,
        "aircraft=\"c172x\"",
        &format!("aircraft=\"{INERTIA}\""),
        source,
    )?;
    with_header(
        &body,
        "runscript",
        &format!(
            "c1722_untrimmed.xml: {source} on {INERTIA} without its Trim\n  event, as JSBSim's tests/TestFuelTanksInertia.py runs it but for the\n  trim, which the port does not have yet."
        ),
        source,
    )
}

/// `unusable_fuel.xml`: `TestUnusableFuel`'s c172x from `reset01` (its
/// engine running) with 1.5 gal of unusable fuel and 10 lbs in each tank,
/// run until the engine starves; at full throttle, so that the fuel above
/// the unusable volume lasts seconds rather than the idle's hour.
#[must_use]
pub fn unusable_fuel_script() -> String {
    format!(
        r#"<?xml version="1.0"?>
<!--
  SPDX-License-Identifier: LGPL-2.1-or-later
  unusable_fuel.xml: JSBSim's tests/TestUnusableFuel.py as a script: the
  c172x ({NOISE0}) from reset01, whose engine runs, with 1.5 gal of
  unusable fuel and 10 lbs in each tank, set before RunIC, run until the
  engine starves; at full throttle, so that it starves within the run.
  Regenerate with `oracle derive`.
-->
<runscript name="unusable fuel">
  <use aircraft="{NOISE0}" initialize="reset01"/>
  <run start="0.0" end="30" dt="0.008333333333333333">
    <property value="1"> fcs/throttle-cmd-norm </property>
    <property value="1.5"> propulsion/tank[0]/unusable-volume-gal </property>
    <property value="1.5"> propulsion/tank[1]/unusable-volume-gal </property>
    <property value="10"> propulsion/tank[0]/contents-lbs </property>
    <property value="10"> propulsion/tank[1]/contents-lbs </property>
  </run>
</runscript>
"#
    )
}

/// The [`INERTIA`] aircraft and the scripts of the running-engine tests.
pub fn files(read: Read<'_>, files: &mut Vec<(String, String)>) -> Result<(), OracleError> {
    let dir = format!("aircraft/{INERTIA}");
    files.push((
        format!("{dir}/{INERTIA}.xml"),
        inertia_aircraft(&read("aircraft/c172x/c172x.xml")?)?,
    ));
    let ap = "aircraft/c172x/c172ap.xml";
    files.push((
        format!("{dir}/c172ap.xml"),
        quiet_autopilot(&read(ap)?, ap)?,
    ));
    files.push((
        format!("{dir}/reset01.xml"),
        read("aircraft/c172x/reset01.xml")?,
    ));
    files.push((
        "scripts/c1722_untrimmed.xml".to_owned(),
        untrimmed_c1722(&read("scripts/c1722.xml")?)?,
    ));
    files.push((
        "scripts/unusable_fuel.xml".to_owned(),
        unusable_fuel_script(),
    ));
    Ok(())
}
