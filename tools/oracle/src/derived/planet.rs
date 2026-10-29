// SPDX-License-Identifier: LGPL-2.1-or-later
//! The tripod of JSBSim's `tests/TestPlanet.py` with a planet file
//! included, as `tests/JSBSim_utils.py`'s
//! `FlightModel.include_planet_test_file` derives it: a
//! `<planet file="...">` element is appended to the upstream
//! `tests/tripod.xml`. The planet file sits next to the aircraft, as a copy
//! of the upstream `tests/` file or, for the geographic error tests, with
//! one radius negated as the test negates it.

use crate::OracleError;

/// One derived tripod: `tripod_<suffix>` includes `file`, a copy of
/// `tests/<file>` with the element `negate` (if any) negated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Planet {
    /// Suffix of the aircraft name.
    pub suffix: &'static str,
    /// The planet file, under `tests/` upstream and next to the aircraft.
    pub file: &'static str,
    /// The radius element whose value the test negates.
    pub negate: Option<&'static str>,
}

/// Every derived planet tripod.
pub const PLANETS: [Planet; 5] = [
    Planet {
        suffix: "moon",
        file: "moon.xml",
        negate: None,
    },
    Planet {
        suffix: "mars",
        file: "mars.xml",
        negate: None,
    },
    Planet {
        suffix: "msis",
        file: "MSIS.xml",
        negate: None,
    },
    Planet {
        suffix: "moon_negative_equatorial",
        file: "moon.xml",
        negate: Some("equatorial_radius"),
    },
    Planet {
        suffix: "moon_negative_polar",
        file: "moon.xml",
        negate: Some("polar_radius"),
    },
];

/// The derived aircraft text from the upstream `tripod.xml` text.
pub fn aircraft(text: &str, planet: &Planet) -> Result<String, OracleError> {
    let fail = |message: &str| OracleError::Provenance(format!("tests/tripod.xml: {message}"));
    let header_at = text
        .find("<fdm_config")
        .ok_or_else(|| fail("no <fdm_config>"))?;
    let close_at = text
        .rfind("</fdm_config>")
        .ok_or_else(|| fail("no </fdm_config>"))?;
    let (suffix, file) = (planet.suffix, planet.file);
    let mut out = String::with_capacity(text.len() + 512);
    out.push_str(&text[..header_at]);
    out.push_str(&format!(
        "<!--\n  SPDX-License-Identifier: LGPL-2.1-or-later\n  tripod_{suffix}: tests/tripod.xml with <planet file=\"{file}\"/>\n  appended, as JSBSim's tests/JSBSim_utils.py include_planet_test_file\n  derives it. Regenerate with `oracle derive`.\n-->\n"
    ));
    out.push_str(&text[header_at..close_at]);
    out.push_str(&format!("  <planet file=\"{file}\"/>\n"));
    out.push_str(&text[close_at..]);
    Ok(out)
}

/// The planet file next to the aircraft: the upstream text, with the
/// value of `negate` negated as `TestPlanet.py`'s geographic error tests
/// do.
pub fn planet_file(text: &str, planet: &Planet) -> Result<String, OracleError> {
    let Some(element) = planet.negate else {
        return Ok(text.to_owned());
    };
    let fail = || OracleError::Provenance(format!("tests/{}: no <{element}>", planet.file));
    let open = text.find(&format!("<{element}")).ok_or_else(fail)?;
    let value_at = open + text[open..].find('>').ok_or_else(fail)? + 1;
    let mut out = String::with_capacity(text.len() + 1);
    out.push_str(&text[..value_at]);
    out.push('-');
    out.push_str(&text[value_at..]);
    Ok(out)
}

/// Paths under `oracle/derived` and contents of every derived planet
/// tripod and its planet file; `read` reads a file of `oracle/inputs`.
pub fn files(
    read: &dyn Fn(&str) -> Result<String, OracleError>,
) -> Result<Vec<(String, String)>, OracleError> {
    let tripod = read("tests/tripod.xml")?;
    let mut files = Vec::new();
    for planet in &PLANETS {
        let suffix = planet.suffix;
        files.push((
            format!("aircraft/tripod_{suffix}/tripod_{suffix}.xml"),
            aircraft(&tripod, planet)?,
        ));
        files.push((
            format!("aircraft/tripod_{suffix}/{}", planet.file),
            planet_file(&read(&format!("tests/{}", planet.file))?, planet)?,
        ));
    }
    Ok(files)
}
