// SPDX-License-Identifier: LGPL-2.1-or-later
//! The tripod of JSBSim's ground reaction tests standing on its contacts:
//!
//! - `tripod_ground` is `tests/tripod.xml` as `tests/TestGndReactions.py`
//!   flies it, from the test's initial conditions (0.1 ft above sea level
//!   at the North pole, which compresses each contact by its share of the
//!   weight), but untrimmed: the `trim` golden runs the test's own trimmed
//!   tripod. Untrimmed, its 1 slug ft2 inertias tumble it off its contacts within 0.2 s at
//!   120 Hz, so it takes the 1000 slug ft2 of `TestMassChangeOnGround.py`
//!   for that reason. `tripod_ground_centered.xml` stands it as
//!   `test_centered_mass`, `tripod_ground_eccentric.xml` with the 1000 lb
//!   point mass of `test_eccentric_mass`.
//! - `tripod_mass_change` is the tripod as `tests/TestMassChangeOnGround.py`
//!   edits it (BOGEY contacts, 1000 slug ft2 inertias, the CG at the
//!   structural origin, the point mass 1 ft below it), and
//!   `tripod_mass_change.xml` runs the test: 240 frames standing 0.1 ft
//!   AGL, then the point mass weighs 1000 lb.

use crate::OracleError;

/// The derived aircraft of `test_centered_mass` and `test_eccentric_mass`.
pub const GROUND: &str = "tripod_ground";
/// The derived aircraft of `TestMassChangeOnGround`.
pub const MASS_CHANGE: &str = "tripod_mass_change";

fn fail(message: &str) -> OracleError {
    OracleError::Provenance(format!("tests/tripod.xml: {message}"))
}

/// `text` with every `from` replaced, which must occur `count` times.
fn replace_all(text: &str, from: &str, to: &str, count: usize) -> Result<String, OracleError> {
    let found = text.matches(from).count();
    if found != count {
        return Err(fail(&format!(
            "{found} {from:?} where {count} were expected"
        )));
    }
    Ok(text.replace(from, to))
}

/// The upstream text with `comment` after the XML declaration.
fn with_header(text: &str, comment: &str) -> Result<String, OracleError> {
    let at = text
        .find("<fdm_config")
        .ok_or_else(|| fail("no <fdm_config>"))?;
    Ok(format!(
        "{}<!--\n{comment}-->\n{}",
        &text[..at],
        &text[at..]
    ))
}

/// `text` with the 1000 slug ft2 inertias of `TestMassChangeOnGround.py`.
fn stiff_inertias(text: &str) -> Result<String, OracleError> {
    let mut out = text.to_owned();
    for axis in ["ixx", "iyy", "izz"] {
        out = replace_all(
            &out,
            &format!("<{axis} unit=\"SLUG*FT2\"> 1.0 </{axis}>"),
            &format!("<{axis} unit=\"SLUG*FT2\">1000.0</{axis}>"),
            1,
        )?;
    }
    Ok(out)
}

/// `tripod_ground.xml` from the upstream `tests/tripod.xml`.
pub fn ground_aircraft(text: &str) -> Result<String, OracleError> {
    with_header(
        &stiff_inertias(text)?,
        "  SPDX-License-Identifier: LGPL-2.1-or-later\n  tripod_ground: tests/tripod.xml as JSBSim's tests/TestGndReactions.py\n  flies it, with the 1000 slug ft2 inertias tests/TestMassChangeOnGround.py\n  gives it to stand untrimmed at 120 Hz. Regenerate with `oracle derive`.\n",
    )
}

/// `tripod_mass_change.xml` from the upstream `tests/tripod.xml`, edited
/// as `TestMassChangeOnGround.structure_jump('BOGEY', 0.0)` edits it.
pub fn mass_change_aircraft(text: &str) -> Result<String, OracleError> {
    let out = replace_all(text, "type=\"STRUCTURE\"", "type=\"BOGEY\"", 3)?;
    let mut out = stiff_inertias(&out)?;
    out = replace_all(
        &out,
        "        <x> 0.25 </x>\n        <y> 0.0 </y>\n        <z> 0.0 </z>",
        "        <x>0.0</x>\n        <y> 0.0 </y>\n        <z>-1.0</z>",
        1,
    )?;
    out = replace_all(
        &out,
        "    </pointmass>\n  </mass_balance>",
        "    </pointmass>\n    <location name=\"CG\" unit=\"FT\"><x>0.0</x><y>0.0</y><z>0.0</z></location>\n  </mass_balance>",
        1,
    )?;
    with_header(
        &out,
        "  SPDX-License-Identifier: LGPL-2.1-or-later\n  tripod_mass_change: tests/tripod.xml as JSBSim's\n  tests/TestMassChangeOnGround.py structure_jump('BOGEY', 0.0) edits it:\n  BOGEY contacts, 1000 slug ft2 inertias, the CG at the structural origin\n  and the point mass 1 ft below it. Regenerate with `oracle derive`.\n",
    )
}

/// The initial conditions of `TestGndReactions.py`.
#[must_use]
pub fn north_pole() -> String {
    r#"<?xml version="1.0"?>
<!--
  SPDX-License-Identifier: LGPL-2.1-or-later
  JSBSim's tests/TestGndReactions.py initial conditions: ic/h-sl-ft 0.1 at
  ic/lat-gc-deg 90, clear of centrifugal and Coriolis accelerations.
  Regenerate with `oracle derive`.
-->
<initialize name="north_pole">
  <latitude unit="DEG" type="geocentric"> 90.0 </latitude>
  <longitude unit="DEG"> 0.0 </longitude>
  <altitude unit="FT"> 0.1 </altitude>
</initialize>
"#
    .to_owned()
}

/// The initial conditions of `TestMassChangeOnGround.py`.
#[must_use]
pub fn mass_change_ic() -> String {
    r#"<?xml version="1.0"?>
<!--
  SPDX-License-Identifier: LGPL-2.1-or-later
  JSBSim's tests/TestMassChangeOnGround.py initial conditions: ic/h-agl-ft
  0.1, the contacts starting compressed by the tripod's weight.
  Regenerate with `oracle derive`.
-->
<initialize name="ground">
  <latitude unit="DEG"> 0.0 </latitude>
  <longitude unit="DEG"> 0.0 </longitude>
  <altitudeAGL unit="FT"> 0.1 </altitudeAGL>
</initialize>
"#
    .to_owned()
}

/// A script standing `aircraft` from `ic` for `end` s at JSBSim's default
/// 120 Hz, with `body` inside its `<run>`.
fn script(name: &str, about: &str, aircraft: &str, ic: &str, end: f64, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!--
  SPDX-License-Identifier: LGPL-2.1-or-later
  {name}.xml: {about} Regenerate with `oracle derive`.
-->
<runscript name="{name}">
  <use aircraft="{aircraft}" initialize="{ic}"/>
  <run start="0.0" end="{end:?}" dt="0.008333333333333333">
{body}  </run>
</runscript>
"#
    )
}

/// The derived scripts, by file name.
#[must_use]
pub fn scripts() -> Vec<(String, String)> {
    let centered = script(
        "tripod_ground_centered",
        "tests/TestGndReactions.py test_centered_mass\n  untrimmed: the tripod standing on its three contacts at the North pole.",
        GROUND,
        "north_pole",
        10.0,
        "",
    );
    let eccentric = script(
        "tripod_ground_eccentric",
        "tests/TestGndReactions.py test_eccentric_mass\n  untrimmed: the tripod with a 1000 lb point mass 0.25 ft off its CG,\n  settling on its contacts at the North pole.",
        GROUND,
        "north_pole",
        10.0,
        "    <property value=\"1000.\"> inertia/pointmass-weight-lbs </property>\n",
    );
    let mass_change = script(
        MASS_CHANGE,
        "tests/TestMassChangeOnGround.py\n  test_cg_at_structural_origin: 240 frames standing, then the 1000 lb point\n  mass 1 ft below the CG, which must not make the structure jump.",
        MASS_CHANGE,
        "ground",
        4.0,
        "    <event name=\"Payload\">\n      <condition> simulation/frame ge 240 </condition>\n      <set name=\"inertia/pointmass-weight-lbs\" value=\"1000.0\"/>\n    </event>\n",
    );
    vec![
        ("tripod_ground_centered.xml".to_owned(), centered),
        ("tripod_ground_eccentric.xml".to_owned(), eccentric),
        (format!("{MASS_CHANGE}.xml"), mass_change),
    ]
}

/// Every derived file of this module, from the upstream `tests/tripod.xml`.
pub fn files(tripod: &str) -> Result<Vec<(String, String)>, OracleError> {
    let mut files = vec![
        (
            format!("aircraft/{GROUND}/{GROUND}.xml"),
            ground_aircraft(tripod)?,
        ),
        (format!("aircraft/{GROUND}/north_pole.xml"), north_pole()),
        (
            format!("aircraft/{MASS_CHANGE}/{MASS_CHANGE}.xml"),
            mass_change_aircraft(tripod)?,
        ),
        (
            format!("aircraft/{MASS_CHANGE}/ground.xml"),
            mass_change_ic(),
        ),
    ];
    for (name, text) in scripts() {
        files.push((format!("scripts/{name}"), text));
    }
    Ok(files)
}
