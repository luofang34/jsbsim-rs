// SPDX-License-Identifier: LGPL-2.1-or-later
//! Corpus aircraft with their tanks and without their engines, for the
//! fuel tank cases of JSBSim's `tests/TestFuelTanksInertia.py` and
//! `tests/TestPointMassInertia.py`, which edit the aircraft as these
//! derivations do.
//!
//! The port refuses every engine type until engines are ported, so each
//! `<engine>` element is removed; the tanks, their feed priorities and the
//! rest of the aircraft stay as upstream wrote them. What the tests read
//! does not depend on a running engine.

use crate::OracleError;

/// One derived aircraft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tanks {
    /// Name of the derived aircraft.
    pub name: &'static str,
    /// The upstream aircraft.
    pub source: &'static str,
    /// What else the test changes, as text replacements applied in order
    /// to the first match after the first `<tank`.
    pub edits: &'static [(&'static str, &'static str)],
    /// Lines removed, by their trimmed text.
    pub drop_lines: &'static [&'static str],
    /// Properties the removed engines and their throttles (or the removed
    /// autopilot) would make, which the rest of the aircraft reads;
    /// declared at the start of `<propulsion>`, at 0.
    pub declares: &'static [&'static str],
    /// What the header says the edits are.
    pub summary: &'static str,
}

/// The upstream contents of J246's first grain.
const GRAIN_CONTENTS: &str = "<contents unit=\"LBS\"> 1114092 </contents>";

/// What J246's guidance reads of its six engines.
const J246_ENGINES: &[&str] = &[
    "fcs/throttle-cmd-norm[0]",
    "propulsion/engine[0]/thrust-lbs",
    "propulsion/engine[1]/thrust-lbs",
    "propulsion/engine[2]/thrust-lbs",
    "propulsion/engine[3]/thrust-lbs",
    "propulsion/engine[4]/thrust-lbs",
    "propulsion/engine[5]/thrust-lbs",
];

/// Every derived tank aircraft.
pub const AIRCRAFT: &[Tanks] = &[
    Tanks {
        name: "c172x_tanks",
        source: "c172x",
        // `test_fuel_tanks_inertia` appends <inertia_factor>1.0 to the
        // first tank.
        edits: &[("</tank>", "<inertia_factor>1.0</inertia_factor></tank>")],
        // The port refuses the sensor noise of its autopilot, which flies
        // nothing the test reads.
        drop_lines: &["<autopilot file=\"c172ap\"/>"],
        declares: &[
            "ap/elevator_cmd",
            "ap/aileron_cmd",
            "ap/roll-cmd-norm-output",
        ],
        summary: "an <inertia_factor> of 1.0 appended to its first tank, and\n  without its autopilot",
    },
    Tanks {
        name: "J246_tanks",
        source: "J246",
        edits: &[],
        drop_lines: &[],
        declares: J246_ENGINES,
        summary: "nothing else changed",
    },
    Tanks {
        name: "J246_half_grain",
        source: "J246",
        // `test_grain_tanks_content` writes str(0.5*capacity) into the
        // contents of the first (CYLINDRICAL) grain.
        edits: &[(GRAIN_CONTENTS, "<contents unit=\"LBS\">557046.0</contents>")],
        drop_lines: &[],
        declares: J246_ENGINES,
        summary: "its first grain's contents set to half its capacity",
    },
    Tanks {
        name: "J246_empty_grain",
        source: "J246",
        edits: &[(GRAIN_CONTENTS, "<contents unit=\"LBS\">0.0</contents>")],
        drop_lines: &[],
        declares: J246_ENGINES,
        summary: "its first grain's contents set to 0",
    },
    Tanks {
        name: "f16_tanks",
        source: "f16",
        edits: &[],
        drop_lines: &[],
        declares: &[
            "fcs/throttle-cmd-norm",
            "fcs/throttle-pos-norm",
            "propulsion/total-fuel-lbs",
            "forces/fbx-prop-lbs",
        ],
        summary: "nothing else changed",
    },
];

/// The files `source` includes from its own directory, which the
/// derived aircraft's directory holds copies of.
#[must_use]
pub fn includes(source: &str) -> &'static [&'static str] {
    match source {
        "J246" => &[
            "Systems/J246GuidanceExecutive.xml",
            "Systems/J246ControlSystem.xml",
            "Systems/J246FirstStageEffectors.xml",
            "Systems/J246SecondStageEffectors.xml",
        ],
        "f16" => &["Systems/pushback.xml", "Systems/hook.xml"],
        _ => &[],
    }
}

/// `text` without its `<engine>` elements, each with the indentation
/// before it and the line end after it.
fn strip_engines(text: &str, source: &str) -> Result<String, OracleError> {
    let fail = |m: &str| OracleError::Provenance(format!("{source}: {m}"));
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<engine") {
        let after = &rest[start..];
        let tag_end = after
            .find('>')
            .ok_or_else(|| fail("unterminated <engine>"))?;
        let end = if after[..tag_end].ends_with('/') {
            tag_end + 1
        } else {
            after
                .find("</engine>")
                .ok_or_else(|| fail("unterminated <engine>"))?
                + "</engine>".len()
        };
        let head = &rest[..start];
        let line_start = head.rfind('\n').map_or(0, |i| i + 1);
        let keep = if head[line_start..].trim().is_empty() {
            line_start
        } else {
            start
        };
        out.push_str(&rest[..keep]);
        let tail = &after[end..];
        rest = tail.strip_prefix('\n').unwrap_or(tail);
    }
    out.push_str(rest);
    Ok(out)
}

/// The derived aircraft text of `tanks` from the upstream text of its
/// source aircraft.
pub fn derive(text: &str, tanks: &Tanks) -> Result<String, OracleError> {
    let source = format!("aircraft/{0}/{0}.xml", tanks.source);
    let fail = |m: &str| OracleError::Provenance(format!("{source}: {m}"));
    let header_at = text
        .find("<fdm_config")
        .ok_or_else(|| fail("no <fdm_config>"))?;
    let mut out = String::with_capacity(text.len() + 512);
    out.push_str(&text[..header_at]);
    out.push_str(&format!(
        "<!--\n  SPDX-License-Identifier: LGPL-2.1-or-later\n  {}: {source} without its engines, and {}.\n  Regenerate with `oracle derive`.\n-->\n",
        tanks.name, tanks.summary
    ));
    let mut body = strip_engines(&text[header_at..], &source)?;
    for (from, to) in tanks.edits {
        let tank_at = body.find("<tank").ok_or_else(|| fail("no <tank>"))?;
        let at = body[tank_at..]
            .find(from)
            .ok_or_else(|| fail(&format!("no {from} in a tank")))?
            + tank_at;
        body.replace_range(at..at + from.len(), to);
    }
    if !tanks.declares.is_empty() {
        let open = body
            .find("<propulsion")
            .ok_or_else(|| fail("no <propulsion>"))?;
        let at = body[open..].find('\n').map_or(body.len(), |i| open + i + 1);
        let indent = body[..open].rsplit('\n').next().unwrap_or_default();
        let declared: String = tanks
            .declares
            .iter()
            .map(|p| format!("{indent}  <property>{p}</property>\n"))
            .collect();
        body.insert_str(at, &declared);
    }
    for line in tanks.drop_lines {
        let at = body.find(line).ok_or_else(|| fail(&format!("no {line}")))?;
        let start = body[..at].rfind('\n').map_or(0, |i| i + 1);
        let end = body[at..].find('\n').map_or(body.len(), |i| at + i + 1);
        body.replace_range(start..end, "");
    }
    out.push_str(&body);
    Ok(out)
}
