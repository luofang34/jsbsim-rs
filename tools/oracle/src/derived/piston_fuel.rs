// SPDX-License-Identifier: LGPL-2.1-or-later
//! The c172x of the tests that start its engine from its initial
//! conditions:
//!
//! - [`INERTIA`]: the c172x with the `<inertia_factor>` that
//!   `TestFuelTanksInertia` adds to its first tank, and `scripts/c1722.xml`
//!   on it;
//! - `unusable_fuel.xml`: `TestUnusableFuel`'s settings as a script.

use super::piston::{C172X, Read, fail, renamed_script, with_header};
use crate::OracleError;

/// The c172x with an inertia factor on its first tank.
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
            "{INERTIA}: {source} with an <inertia_factor> of 1.0 appended to\n  its first tank, as JSBSim's tests/TestFuelTanksInertia.py edits it."
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
  c172x from reset01, whose engine runs, with 1.5 gal of
  unusable fuel and 10 lbs in each tank, set before RunIC, run until the
  engine starves; at full throttle, so that it starves within the run.
  Regenerate with `oracle derive`.
-->
<runscript name="unusable fuel">
  <use aircraft="{C172X}" initialize="reset01"/>
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
    files.push((format!("{dir}/c172ap.xml"), read(ap)?));
    files.push((
        format!("{dir}/reset01.xml"),
        read("aircraft/c172x/reset01.xml")?,
    ));
    files.push((
        "scripts/c1722_inertia.xml".to_owned(),
        renamed_script(&read("scripts/c1722.xml")?, "c1722", "c172x", INERTIA)?,
    ));
    files.push((
        "scripts/unusable_fuel.xml".to_owned(),
        unusable_fuel_script(),
    ));
    Ok(())
}
