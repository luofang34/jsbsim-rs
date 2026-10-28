// SPDX-License-Identifier: LGPL-2.1-or-later
//! The tripod aircraft of JSBSim's Python FCS tests with one system
//! included, as `tests/JSBSim_utils.py`'s
//! `FlightModel.include_system_test_file` derives it: a
//! `<system file="...">` element is appended to the upstream
//! `tests/tripod.xml`, and the system is found on the systems path.

use crate::OracleError;

/// One derived tripod: `tripod_<suffix>` includes `file`, found in
/// `systems_path` (relative to `oracle/`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct System {
    /// Suffix of the aircraft name.
    pub suffix: &'static str,
    /// The `file` attribute of the included `<system>`.
    pub file: &'static str,
    /// Systems path the file is found on, relative to `oracle/`.
    pub systems_path: &'static str,
    /// Whether JSBSim loads it; one exists to fail as an upstream test
    /// expects.
    pub loads: bool,
}

/// Every derived tripod.
pub const SYSTEMS: [System; 7] = [
    upstream("gain", "gain.xml"),
    upstream("deadband", "deadband.xml"),
    derived("function", "function_no_random.xml"),
    System {
        loads: false,
        ..derived("function_bad_table", "function_bad_table.xml")
    },
    scenario("fcs_base", "fcs_base.xml"),
    scenario("fcs_gain", "fcs_gain.xml"),
    scenario("fcs_summer_deadband", "fcs_summer_deadband.xml"),
];

/// A tripod including a system of `oracle/derived/systems`
/// ([`super::systems`]).
const fn derived(suffix: &'static str, file: &'static str) -> System {
    System {
        suffix,
        file,
        systems_path: "derived/systems",
        loads: true,
    }
}

/// A tripod including an upstream `tests/` system, vendored in
/// `oracle/inputs/tests`.
const fn upstream(suffix: &'static str, file: &'static str) -> System {
    System {
        suffix,
        file,
        systems_path: "inputs/tests",
        loads: true,
    }
}

/// A tripod including a system of `oracle/scenarios/systems`.
const fn scenario(suffix: &'static str, file: &'static str) -> System {
    System {
        suffix,
        file,
        systems_path: "scenarios/systems",
        loads: true,
    }
}

/// The derived aircraft text from the upstream `tripod.xml` text.
pub fn derive(text: &str, suffix: &str, system: &str) -> Result<String, OracleError> {
    let fail = |message: &str| OracleError::Provenance(format!("tests/tripod.xml: {message}"));
    let header_at = text
        .find("<fdm_config")
        .ok_or_else(|| fail("no <fdm_config>"))?;
    let close_at = text
        .rfind("</fdm_config>")
        .ok_or_else(|| fail("no </fdm_config>"))?;
    let mut out = String::with_capacity(text.len() + 512);
    out.push_str(&text[..header_at]);
    out.push_str(&format!(
        "<!--\n  SPDX-License-Identifier: LGPL-2.1-or-later\n  tripod_{suffix}: tests/tripod.xml with <system file=\"{system}\"/>\n  appended, as JSBSim's tests/JSBSim_utils.py include_system_test_file\n  derives it. Regenerate with `oracle derive`.\n-->\n"
    ));
    out.push_str(&text[header_at..close_at]);
    out.push_str(&format!("  <system file=\"{system}\"/>\n"));
    out.push_str(&text[close_at..]);
    Ok(out)
}
