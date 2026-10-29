// SPDX-License-Identifier: LGPL-2.1-or-later
//! Turbine and turboprop aircraft and scripts derived from upstream ones
//! as JSBSim's Python tests derive them:
//!
//! - [`SPOOLS`]: the f16 of `TestTurbine.testSpoolUp`, its engine with a
//!   `<function>` of `tests/N1SpoolUp.xml` appended under each spool
//!   name in turn, with `scripts/f16_test.xml` on each;
//! - [`POWER_VC_FUNCTION`]: the L410 of `TestTurboProp.testEnginePowerVC`,
//!   its engine's `EnginePowerVC` table moved into a `<function>`, with
//!   `scripts/L4102.xml` on it;
//! - [`WESTWARD`]: the 737 of `CheckTrim.test_trim_westward`, its cruise
//!   initial conditions heading 270°, with `scripts/737_cruise.xml` on it.

use super::piston::{Read, fail, renamed_script, replace_once, with_header};
use crate::OracleError;

/// The spool functions `testSpoolUp` defines in turn.
pub const SPOOLS: [&str; 4] = ["N1SpoolUp", "N1SpoolDown", "N2SpoolUp", "N2SpoolDown"];

/// The L410 whose `EnginePowerVC` is a function.
pub const POWER_VC_FUNCTION: &str = "L410_function";

/// The 737 heading west.
pub const WESTWARD: &str = "737_westward";

const F100: &str = "engine/F100-PW-229.xml";
const ENGTM601: &str = "engine/engtm601.xml";

/// The f16's engine with the spool function `name` appended, its body
/// `tests/N1SpoolUp.xml`.
pub fn spool_engine(engine: &str, body: &str, name: &str) -> Result<String, OracleError> {
    let close = engine
        .rfind("</turbine_engine>")
        .ok_or_else(|| fail(F100, "no </turbine_engine>"))?;
    let body = body.trim_start_matches("<?xml version=\"1.0\"?>").trim();
    let mut out = engine.to_owned();
    out.insert_str(
        close,
        &format!("  <function name=\"{name}\">\n    {body}\n  </function>\n"),
    );
    with_header(
        &out,
        "turbine_engine",
        &format!(
            "F100-PW-229_{name}.xml: {F100} with <function name=\"{name}\">\n  of tests/N1SpoolUp.xml appended, as JSBSim's tests/TestTurbine.py\n  derives it."
        ),
        F100,
    )
}

/// An aircraft `text` (`source`) renamed `name`, with each engine file
/// `from` replaced by `to`.
fn engine_renamed(
    text: &str,
    source: &str,
    name: &str,
    from: &str,
    to: &str,
) -> Result<String, OracleError> {
    let tag = format!("<engine file=\"{from}\">");
    if !text.contains(&tag) {
        return Err(fail(source, &format!("no {tag}")));
    }
    let body = text.replace(&tag, &format!("<engine file=\"{to}\">"));
    with_header(
        &body,
        "fdm_config",
        &format!("{name}: {source} on engine/{to}.xml."),
        source,
    )
}

/// The L410's engine with its `EnginePowerVC` table moved, without its
/// name, into `<function name="EnginePowerVC">` at the end.
pub fn power_vc_engine(engine: &str) -> Result<String, OracleError> {
    let start = engine
        .find("<table name=\"EnginePowerVC\">")
        .ok_or_else(|| fail(ENGTM601, "no EnginePowerVC table"))?;
    let end = engine[start..]
        .find("</table>")
        .ok_or_else(|| fail(ENGTM601, "unterminated EnginePowerVC table"))?
        + start
        + "</table>".len();
    let table = engine[start..end].replacen(" name=\"EnginePowerVC\"", "", 1);
    let mut out = format!("{}{}", &engine[..start], &engine[end..]);
    let close = out
        .rfind("</turboprop_engine>")
        .ok_or_else(|| fail(ENGTM601, "no </turboprop_engine>"))?;
    out.insert_str(
        close,
        &format!("  <function name=\"EnginePowerVC\">\n  {table}\n  </function>\n"),
    );
    with_header(
        &out,
        "turboprop_engine",
        &format!(
            "engtm601_function.xml: {ENGTM601} with its EnginePowerVC table in a\n  <function>, as JSBSim's tests/TestTurboProp.py derives it."
        ),
        ENGTM601,
    )
}

/// The 737's cruise initial conditions heading 270°.
pub fn westward_init(text: &str) -> Result<String, OracleError> {
    let source = "aircraft/737/cruise_init.xml";
    let body = replace_once(
        text,
        "<psi unit=\"DEG\">       225.0 </psi>",
        "<psi unit=\"DEG\">270.0</psi>",
        source,
    )?;
    with_header(
        &body,
        "initialize",
        &format!(
            "{WESTWARD}: {source} heading 270°, as JSBSim's\n  tests/CheckTrim.py derives it."
        ),
        source,
    )
}

fn copy(
    read: Read<'_>,
    files: &mut Vec<(String, String)>,
    from: &str,
    to: &str,
) -> Result<(), OracleError> {
    files.push((to.to_owned(), read(from)?));
    Ok(())
}

/// The f16 variants of [`SPOOLS`].
fn spool_files(read: Read<'_>, files: &mut Vec<(String, String)>) -> Result<(), OracleError> {
    let engine = read(F100)?;
    let body = read("tests/N1SpoolUp.xml")?;
    let aircraft = read("aircraft/f16/f16.xml")?;
    let script = read("scripts/f16_test.xml")?;
    copy(read, files, "engine/direct.xml", "engine/direct.xml")?;
    for spool in SPOOLS {
        let name = format!("f16_{spool}");
        let dir = format!("aircraft/{name}");
        let engine_name = format!("F100-PW-229_{spool}");
        files.push((
            format!("engine/{engine_name}.xml"),
            spool_engine(&engine, &body, spool)?,
        ));
        files.push((
            format!("{dir}/{name}.xml"),
            engine_renamed(
                &aircraft,
                "aircraft/f16/f16.xml",
                &name,
                "F100-PW-229",
                &engine_name,
            )?,
        ));
        for f in ["reset00.xml", "Systems/hook.xml", "Systems/pushback.xml"] {
            copy(
                read,
                files,
                &format!("aircraft/f16/{f}"),
                &format!("{dir}/{f}"),
            )?;
        }
        files.push((
            format!("scripts/f16_test_{spool}.xml"),
            spool_script(&script, &name)?,
        ));
    }
    Ok(())
}

/// `scripts/f16_test.xml` on `aircraft`, cutting the throttle once N2
/// reaches 100%, as `testSpoolUp` does to spool the engine down.
pub fn spool_script(text: &str, aircraft: &str) -> Result<String, OracleError> {
    let source = "scripts/f16_test.xml";
    let body = replace_once(
        text,
        "aircraft=\"f16\"",
        &format!("aircraft=\"{aircraft}\""),
        source,
    )?;
    let body = replace_once(
        &body,
        "  </run>",
        "    <event name=\"spool down\">\n      <condition> propulsion/engine[0]/n2 ge 100 </condition>\n      <set name=\"fcs/throttle-cmd-norm[0]\" value=\"0\"/>\n    </event>\n  </run>",
        source,
    )?;
    with_header(
        &body,
        "runscript",
        &format!(
            "f16_test_{}.xml: {source} on {aircraft}, cutting the throttle once\n  N2 reaches 100%, as JSBSim's tests/TestTurbine.py does.",
            aircraft.trim_start_matches("f16_")
        ),
        source,
    )
}

/// Every file of this module, by its path under `oracle/derived`.
pub fn files(read: Read<'_>, files: &mut Vec<(String, String)>) -> Result<(), OracleError> {
    spool_files(read, files)?;
    let dir = format!("aircraft/{POWER_VC_FUNCTION}");
    files.push((
        "engine/engtm601_function.xml".to_owned(),
        power_vc_engine(&read(ENGTM601)?)?,
    ));
    copy(read, files, "engine/vrtule2.xml", "engine/vrtule2.xml")?;
    files.push((
        format!("{dir}/{POWER_VC_FUNCTION}.xml"),
        engine_renamed(
            &read("aircraft/L410/L410.xml")?,
            "aircraft/L410/L410.xml",
            POWER_VC_FUNCTION,
            "engtm601",
            "engtm601_function",
        )?,
    ));
    copy(
        read,
        files,
        "aircraft/L410/reset00.xml",
        &format!("{dir}/reset00.xml"),
    )?;
    files.push((
        "scripts/L4102_function.xml".to_owned(),
        renamed_script(
            &read("scripts/L4102.xml")?,
            "L4102",
            "L410",
            POWER_VC_FUNCTION,
        )?,
    ));
    let dir = format!("aircraft/{WESTWARD}");
    let source = "aircraft/737/737.xml";
    files.push((
        format!("{dir}/{WESTWARD}.xml"),
        with_header(
            &read(source)?,
            "fdm_config",
            &format!("{WESTWARD}: {source}, unchanged; its cruise_init.xml heads west."),
            source,
        )?,
    ));
    files.push((
        format!("{dir}/cruise_init.xml"),
        westward_init(&read("aircraft/737/cruise_init.xml")?)?,
    ));
    copy(read, files, "engine/CFM56.xml", "engine/CFM56.xml")?;
    copy(
        read,
        files,
        "scripts/unitconversions.xml",
        "scripts/unitconversions.xml",
    )?;
    files.push((
        "scripts/737_cruise_westward.xml".to_owned(),
        renamed_script(
            &read("scripts/737_cruise.xml")?,
            "737_cruise",
            "737",
            WESTWARD,
        )?,
    ));
    Ok(())
}
