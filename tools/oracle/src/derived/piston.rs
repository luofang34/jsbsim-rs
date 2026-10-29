// SPDX-License-Identifier: LGPL-2.1-or-later
//! Piston aircraft and scripts derived from upstream ones:
//!
//! - [`NOISE0`]: the c172x with its autopilot's sensor noise set to 0,
//!   which the port refuses until noise is ported, with the upstream
//!   scripts that fly the c172x renamed onto it; the `piston_takeoff`
//!   check case gets the same edit;
//! - [`ACCELEROMETERS`]: the c172r with `systems/accelerometers.xml`, and
//!   `scripts/c1721.xml` on it with what `TestAccelerometer.testOnGround`
//!   sets before `RunIC`;
//! - the Short S.23 of `TestEngineIndexedProps`, whose engine's
//!   `starter-max-power-W` function becomes a post-function, and in the
//!   second variant reads an indexed table;
//! - the c172x of the fuel tests, in [`piston_fuel`](super::piston_fuel).

use crate::OracleError;

/// The c172x without sensor noise.
pub const NOISE0: &str = "c172x_noise0";

/// The c172r with accelerometers.
pub const ACCELEROMETERS: &str = "c172r_accelerometers";

/// Upstream scripts flying the c172x untrimmed.
pub const NOISE0_SCRIPTS: [&str; 4] = [
    "c1723",
    "c1724",
    "c172_elevation_test",
    "c172_runway_at_rest_cg_shift",
];

/// The c172x files besides the aircraft and its autopilot.
pub const C172X_FILES: [&str; 4] = [
    "reset00.xml",
    "reset01.xml",
    "reset_at_rest.xml",
    "elevator_doublet_init.xml",
];

/// One Short S.23 variant of `TestEngineIndexedProps`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexedProps {
    /// Suffix of the aircraft, engine and script names.
    pub suffix: &'static str,
    /// What the test changes in the engine's function.
    pub summary: &'static str,
    /// Whether the function's `<difference>` becomes an indexed table.
    pub table: bool,
}

/// The variants: `testFunctionWithIndexedProps` and
/// `testTableWithIndexedVars`.
pub const INDEXED_PROPS: [IndexedProps; 2] = [
    IndexedProps {
        suffix: "post",
        summary: "its starter-max-power-W function a post-function",
        table: false,
    },
    IndexedProps {
        suffix: "table",
        summary: "its starter-max-power-W function a post-function whose\n  difference is a table over the engine's indexed RPM",
        table: true,
    },
];

/// The Short S.23 files besides the aircraft.
pub const SHORT_S23_SYSTEMS: [&str; 7] = [
    "Systems/datcom_aero.xml",
    "Systems/electrical.xml",
    "Systems/engines.xml",
    "Systems/flaps.xml",
    "Systems/fuel-system.xml",
    "Systems/Short_S23-hydrodynamics.xml",
    "Systems/take-off-ap.xml",
];

pub(super) fn fail(source: &str, message: &str) -> OracleError {
    OracleError::Provenance(format!("{source}: {message}"))
}

/// `text` with the comment `comment` before its root element `root`.
pub(super) fn with_header(
    text: &str,
    root: &str,
    comment: &str,
    source: &str,
) -> Result<String, OracleError> {
    let at = text
        .find(&format!("<{root}"))
        .ok_or_else(|| fail(source, &format!("no <{root}>")))?;
    let mut out = String::with_capacity(text.len() + comment.len() + 64);
    out.push_str(&text[..at]);
    out.push_str("<!--\n  SPDX-License-Identifier: LGPL-2.1-or-later\n  ");
    out.push_str(comment);
    out.push_str(" Regenerate with `oracle derive`.\n-->\n");
    out.push_str(&text[at..]);
    Ok(out)
}

/// `text` with its only `from` replaced by `to`.
pub(super) fn replace_once(
    text: &str,
    from: &str,
    to: &str,
    source: &str,
) -> Result<String, OracleError> {
    if text.matches(from).count() != 1 {
        return Err(fail(source, &format!("expected one {from}")));
    }
    Ok(text.replacen(from, to, 1))
}

/// An autopilot (`c172ap.xml`) with its sensor noise set to 0.
pub fn quiet_autopilot(text: &str, source: &str) -> Result<String, OracleError> {
    let body = replace_once(text, "> 0.05 </noise>", "> 0 </noise>", source)?;
    with_header(
        &body,
        "autopilot",
        &format!(
            "{source} with its sensor <noise> set to 0, which the port\n  refuses until sensor noise is ported."
        ),
        source,
    )
}

/// The [`NOISE0`] aircraft from the upstream c172x.
pub fn noise0_aircraft(text: &str) -> Result<String, OracleError> {
    with_header(
        text,
        "fdm_config",
        &format!(
            "{NOISE0}: aircraft/c172x/c172x.xml, unchanged; its autopilot\n  c172ap.xml has its sensor noise set to 0."
        ),
        "aircraft/c172x/c172x.xml",
    )
}

/// Upstream script `name` flying `from`, renamed onto `to`.
pub fn renamed_script(text: &str, name: &str, from: &str, to: &str) -> Result<String, OracleError> {
    let source = format!("scripts/{name}.xml");
    let body = replace_once(
        text,
        &format!("aircraft=\"{from}\""),
        &format!("aircraft=\"{to}\""),
        &source,
    )?;
    with_header(
        &body,
        "runscript",
        &format!("{name}.xml: {source} on {to}."),
        &source,
    )
}

/// The [`ACCELEROMETERS`] aircraft: the c172r with
/// `<system file="accelerometers"/>` appended, as
/// `TestAccelerometer.AddAccelerometersToAircraft` derives it.
pub fn accelerometers_aircraft(text: &str) -> Result<String, OracleError> {
    let source = "aircraft/c172r/c172r.xml";
    let close = text
        .rfind("</fdm_config>")
        .ok_or_else(|| fail(source, "no </fdm_config>"))?;
    let mut body = text.to_owned();
    body.insert_str(close, "  <system file=\"accelerometers\"/>\n");
    with_header(
        &body,
        "fdm_config",
        &format!(
            "{ACCELEROMETERS}: {source} with <system file=\"accelerometers\"/>\n  appended, as JSBSim's tests/TestAccelerometer.py derives it."
        ),
        source,
    )
}

/// `c1721_accelerometers.xml`: `scripts/c1721.xml` on [`ACCELEROMETERS`]
/// with what `testOnGround` sets before `RunIC`: the accelerometer on,
/// the standard gravity model and a heading of 0.
pub fn accelerometers_script(text: &str) -> Result<String, OracleError> {
    let source = "scripts/c1721.xml";
    let body = replace_once(
        text,
        "aircraft=\"c172r\"",
        &format!("aircraft=\"{ACCELEROMETERS}\""),
        source,
    )?;
    let run = "dt=\"0.008333\">\n";
    let body = replace_once(
        &body,
        run,
        &format!(
            "{run}    <property value=\"1\"> fcs/accelerometer/on </property>\n    <property value=\"0\"> simulation/gravity-model </property>\n    <property value=\"0\"> ic/psi-true-rad </property>\n"
        ),
        source,
    )?;
    with_header(
        &body,
        "runscript",
        &format!(
            "c1721_accelerometers.xml: {source} on {ACCELEROMETERS} with the\n  settings of JSBSim's tests/TestAccelerometer.py testOnGround."
        ),
        source,
    )
}

/// The Short S.23 of `variant`, with its engines' file renamed.
pub fn indexed_aircraft(text: &str, variant: &IndexedProps) -> Result<String, OracleError> {
    let source = "aircraft/Short_S23/Short_S23.xml";
    let suffix = variant.suffix;
    if text.matches("<engine file=\"eng_PegasusXc\">").count() != 4 {
        return Err(fail(source, "expected four eng_PegasusXc engines"));
    }
    let body = text.replace(
        "<engine file=\"eng_PegasusXc\">",
        &format!("<engine file=\"eng_PegasusXc_{suffix}\">"),
    );
    with_header(
        &body,
        "fdm_config",
        &format!("Short_S23_{suffix}: {source} on engine/eng_PegasusXc_{suffix}.xml."),
        source,
    )
}

const STARTER_FUNCTION: &str = "<function name=\"propulsion/engine[#]/starter-max-power-W\">";

const STARTER_DIFFERENCE: &str = "    <difference>
     <value>1.0</value>
     <quotient>
      <property>propulsion/engine[#]/engine-rpm</property>
      <value>400</value>   <!-- Starter max RPM. -->
     </quotient>
    </difference>
";

const STARTER_TABLE: &str = "    <table name=\"propulsion/engine[#]/starter-tabular-data\">
     <independentVar lookup=\"row\">propulsion/engine[#]/engine-rpm</independentVar>
     <tableData>
      0.0 1.0
      400.0 0.0
     </tableData>
    </table>
";

/// The Short S.23's engine edited as `variant`'s test edits it.
pub fn indexed_engine(text: &str, variant: &IndexedProps) -> Result<String, OracleError> {
    let source = "engine/eng_PegasusXc.xml";
    let post = STARTER_FUNCTION.replace('>', " type=\"post\">");
    let mut body = replace_once(text, STARTER_FUNCTION, &post, source)?;
    if variant.table {
        body = replace_once(&body, STARTER_DIFFERENCE, STARTER_TABLE, source)?;
    }
    with_header(
        &body,
        "piston_engine",
        &format!(
            "eng_PegasusXc_{}.xml: {source} with {}, as JSBSim's\n  tests/TestEngineIndexedProps.py edits it.",
            variant.suffix, variant.summary
        ),
        source,
    )
}

/// Reads a vendored upstream file by its path under `oracle/inputs`.
pub type Read<'a> = &'a dyn Fn(&str) -> Result<String, OracleError>;

fn copies(
    read: Read<'_>,
    files: &mut Vec<(String, String)>,
    rels: &[&str],
) -> Result<(), OracleError> {
    for rel in rels {
        files.push(((*rel).to_owned(), read(rel)?));
    }
    Ok(())
}

/// The [`NOISE0`] aircraft, its engine and systems, and its scripts.
fn noise0_files(read: Read<'_>, files: &mut Vec<(String, String)>) -> Result<(), OracleError> {
    let dir = format!("aircraft/{NOISE0}");
    files.push((
        format!("{dir}/{NOISE0}.xml"),
        noise0_aircraft(&read("aircraft/c172x/c172x.xml")?)?,
    ));
    let ap = "aircraft/c172x/c172ap.xml";
    files.push((
        format!("{dir}/c172ap.xml"),
        quiet_autopilot(&read(ap)?, ap)?,
    ));
    for f in C172X_FILES {
        files.push((format!("{dir}/{f}"), read(&format!("aircraft/c172x/{f}"))?));
    }
    copies(
        read,
        files,
        &[
            "engine/eng_io320.xml",
            "engine/prop_75in2f.xml",
            "systems/GNCUtilities.xml",
            "systems/Autopilot.xml",
        ],
    )?;
    for name in NOISE0_SCRIPTS {
        let text = read(&format!("scripts/{name}.xml"))?;
        files.push((
            format!("scripts/{name}.xml"),
            renamed_script(&text, name, "c172x", NOISE0)?,
        ));
    }
    Ok(())
}

/// The `piston_takeoff` check case, its autopilot without noise.
fn takeoff_files(read: Read<'_>, files: &mut Vec<(String, String)>) -> Result<(), OracleError> {
    let root = "check_cases/piston_takeoff";
    for rel in [
        "aircraft/c172x/c172x.xml",
        "aircraft/c172x/reset00.xml",
        "engine/eng_io320.xml",
        "engine/prop_Clark_Y7570.xml",
        "systems/GNCUtilities.xml",
        "systems/Autopilot.xml",
        "scripts/c1723.xml",
    ] {
        let path = format!("{root}/{rel}");
        files.push((path.clone(), read(&path)?));
    }
    let ap = format!("{root}/aircraft/c172x/c172ap.xml");
    files.push((ap.clone(), quiet_autopilot(&read(&ap)?, &ap)?));
    Ok(())
}

/// The [`ACCELEROMETERS`] aircraft and its script, and the Short S.23
/// variants of [`INDEXED_PROPS`].
fn test_files(read: Read<'_>, files: &mut Vec<(String, String)>) -> Result<(), OracleError> {
    let dir = format!("aircraft/{ACCELEROMETERS}");
    files.push((
        format!("{dir}/{ACCELEROMETERS}.xml"),
        accelerometers_aircraft(&read("aircraft/c172r/c172r.xml")?)?,
    ));
    files.push((
        format!("{dir}/reset00.xml"),
        read("aircraft/c172r/reset00.xml")?,
    ));
    files.push((
        "scripts/c1721_accelerometers.xml".to_owned(),
        accelerometers_script(&read("scripts/c1721.xml")?)?,
    ));
    copies(
        read,
        files,
        &[
            "engine/engIO360C.xml",
            "engine/prop_Clark_Y7570.xml",
            "engine/prop_deHavilland5000.xml",
            "systems/hydrodynamics.xml",
            "systems/hydrodynamic-planing-floats.xml",
            "systems/sperry-a2-autopilot.xml",
        ],
    )?;
    let aircraft = read("aircraft/Short_S23/Short_S23.xml")?;
    let engine = read("engine/eng_PegasusXc.xml")?;
    let script = read("scripts/Short_S23_1.xml")?;
    for v in &INDEXED_PROPS {
        let name = format!("Short_S23_{}", v.suffix);
        files.push((
            format!("aircraft/{name}/{name}.xml"),
            indexed_aircraft(&aircraft, v)?,
        ));
        for f in SHORT_S23_SYSTEMS.iter().chain(&["reset00.xml"]) {
            files.push((
                format!("aircraft/{name}/{f}"),
                read(&format!("aircraft/Short_S23/{f}"))?,
            ));
        }
        files.push((
            format!("engine/eng_PegasusXc_{}.xml", v.suffix),
            indexed_engine(&engine, v)?,
        ));
        files.push((
            format!("scripts/Short_S23_1_{}.xml", v.suffix),
            renamed_script(&script, "Short_S23_1", "Short_S23", &name)?,
        ));
    }
    Ok(())
}

/// Every file of this module, by its path under `oracle/derived`.
pub fn files(read: Read<'_>) -> Result<Vec<(String, String)>, OracleError> {
    let mut files = Vec::new();
    noise0_files(read, &mut files)?;
    takeoff_files(read, &mut files)?;
    test_files(read, &mut files)?;
    super::piston_fuel::files(read, &mut files)?;
    Ok(files)
}
