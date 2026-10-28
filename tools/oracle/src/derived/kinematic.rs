// SPDX-License-Identifier: LGPL-2.1-or-later
//! The kinematic components of JSBSim's `tests/TestKinematic.py` as
//! systems for the tripod.
//!
//! The test flies the c172r (flaps) and the p51d (landing gear), which the
//! port cannot load yet (their engines), so each system is the channel of
//! the aircraft's `<flight_control>` holding the kinematic the test drives,
//! copied verbatim; `testKinematicNoScale`'s copy has `<noscale/>`
//! appended to the kinematic, as the test appends it.

use crate::OracleError;

/// One derived system: its file name, the vendored aircraft it comes from
/// (under `oracle/inputs`), the channel and whether `<noscale/>` is
/// appended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KinematicSystem {
    /// File name under `oracle/derived/systems`.
    pub file: &'static str,
    /// The aircraft file, relative to `oracle/inputs`.
    pub source: &'static str,
    /// The `name` of the channel copied.
    pub channel: &'static str,
    /// Whether `<noscale/>` is appended to the channel's kinematic.
    pub noscale: bool,
}

/// Every derived kinematic system.
pub const SYSTEMS: [KinematicSystem; 3] = [
    KinematicSystem {
        file: "kinematic_c172r.xml",
        source: "aircraft/c172r/c172r.xml",
        channel: "Flaps",
        noscale: false,
    },
    KinematicSystem {
        file: "kinematic_c172r_noscale.xml",
        source: "aircraft/c172r/c172r.xml",
        channel: "Flaps",
        noscale: true,
    },
    KinematicSystem {
        file: "kinematic_p51d.xml",
        source: "aircraft/p51d/p51d.xml",
        channel: "Landing Gear",
        noscale: false,
    },
];

/// The text of `<channel name="{name}">` in `<flight_control>`, from the
/// start of its line.
fn channel<'a>(text: &'a str, source: &str, name: &str) -> Result<&'a str, OracleError> {
    let fail = |message: &str| OracleError::Provenance(format!("{source}: {message}"));
    let fcs_at = text
        .find("<flight_control")
        .ok_or_else(|| fail("no <flight_control>"))?;
    let open = format!("<channel name=\"{name}\">");
    let start = fcs_at
        + text[fcs_at..]
            .find(&open)
            .ok_or_else(|| fail(&format!("no {open}")))?;
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let end = start
        + text[start..]
            .find("</channel>")
            .ok_or_else(|| fail(&format!("unterminated {open}")))?
        + "</channel>".len();
    Ok(&text[line_start..end])
}

/// The derived system text of `system` from its aircraft's `text`.
pub fn derive(text: &str, system: &KinematicSystem) -> Result<String, OracleError> {
    let source = system.source;
    let mut body = channel(text, source, system.channel)?.to_owned();
    if system.noscale {
        let close = body.find("</kinematic>").ok_or_else(|| {
            OracleError::Provenance(format!(
                "{source}: no <kinematic> in channel {}",
                system.channel
            ))
        })?;
        body.insert_str(close, "<noscale/>");
    }
    let how = if system.noscale {
        "with <noscale/> appended to its kinematic"
    } else {
        "copied verbatim"
    };
    Ok(format!(
        "<?xml version=\"1.0\"?>\n<!--\n  SPDX-License-Identifier: LGPL-2.1-or-later\n  {}: channel \"{}\" of {source},\n  {how}, as JSBSim's tests/TestKinematic.py drives it.\n  Regenerate with `oracle derive`.\n-->\n<system name=\"{}\">\n{body}\n</system>\n",
        system.file,
        system.channel,
        system.file.trim_end_matches(".xml"),
    ))
}
