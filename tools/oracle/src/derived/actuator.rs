// SPDX-License-Identifier: LGPL-2.1-or-later
//! The actuator of JSBSim's `tests/TestActuator.py` in every configuration
//! the test edits it into, as one system for the tripod.
//!
//! The test edits `fcs/left-aileron-actuator` of the c172x (the first
//! actuator with a `<rate_limit>`) and flies the whole aircraft through
//! `scripts/c1724.xml`. The port cannot load the c172x yet (its engine),
//! so each configuration is a copy of that actuator element, with the
//! test's edits applied, in a channel of its own. Every property the
//! actuator names gets the configuration's prefix (`fcs/<label>/...`
//! for `fcs/...`), so the copies never share state; the properties the
//! test adds to `<flight_control>` are declared with the same prefix.

use crate::OracleError;

/// One configuration of the actuator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Prefix label: the actuator becomes `fcs/<label>/left-aileron-actuator`.
    pub label: &'static str,
    /// Whether `prepare_actuator`'s edits apply: no hysteresis, and the
    /// input read from `fcs/<label>/left-aileron-input`, declared 0.
    pub prepared: bool,
    /// Text replacements after those, in order; `{p}` stands for
    /// `fcs/<label>`.
    pub edits: &'static [(&'static str, &'static str)],
    /// Properties declared besides the input: (path under `{p}/`, value).
    pub properties: &'static [(&'static str, Option<&'static str>)],
}

const RATE: &str = "<rate_limit> 1.57 </rate_limit>";
const CLIP_MIN: &str = "<min> -0.350 </min>";
const CLIP_MAX: &str = "<max>  0.260 </max>";
const CLOSE: &str = "</actuator>";
const RATE_PROPERTIES: &[(&str, Option<&str>)] = &[
    ("rate-limit-value", Some("0.15")),
    ("rate-limit-value2", Some("0.05")),
];
const CLIP_PROPERTIES: &[(&str, Option<&str>)] = &[
    ("clip-min-value", Some("-0.15")),
    ("clip-max-value", Some("0.05")),
];

const fn prepared(
    label: &'static str,
    edits: &'static [(&'static str, &'static str)],
    properties: &'static [(&'static str, Option<&'static str>)],
) -> Config {
    Config {
        label,
        prepared: true,
        edits,
        properties,
    }
}

/// Every configuration, in the test's order.
pub const CONFIGS: [Config; 18] = [
    // test_regression_bug_1503: the rate limit split into a decreasing
    // one and an increasing one of half its value.
    Config {
        label: "bug1503",
        prepared: false,
        edits: &[
            (RATE, "<rate_limit sense=\"decr\"> 1.57 </rate_limit>"),
            (
                CLOSE,
                "<rate_limit sense=\"incr\">0.785</rate_limit></actuator>",
            ),
        ],
        properties: &[("left-aileron-control", None)],
    },
    // test_actuator_rate_from_property: both limits read from a property.
    Config {
        label: "rateprop",
        prepared: false,
        edits: &[
            (
                RATE,
                "<rate_limit sense=\"decr\">{p}/rate-limit-value</rate_limit>",
            ),
            (
                CLOSE,
                "<rate_limit sense=\"incr\">{p}/rate-limit-value</rate_limit></actuator>",
            ),
        ],
        properties: &[
            ("left-aileron-control", None),
            ("rate-limit-value", Some(" 1.57 ")),
        ],
    },
    // test_actuator_rate_is_linear, cumulatively.
    prepared(
        "rate1",
        &[(RATE, "<rate_limit>0.1</rate_limit>")],
        RATE_PROPERTIES,
    ),
    prepared(
        "rate2",
        &[(RATE, "<rate_limit>{p}/rate-limit-value</rate_limit>")],
        RATE_PROPERTIES,
    ),
    prepared(
        "rate3",
        &[
            (RATE, "<rate_limit sense=\"decr\">0.1</rate_limit>"),
            (
                CLOSE,
                "<rate_limit sense=\"incr\">0.2</rate_limit></actuator>",
            ),
        ],
        RATE_PROPERTIES,
    ),
    prepared(
        "rate4",
        &[
            (
                RATE,
                "<rate_limit sense=\"decr\">{p}/rate-limit-value</rate_limit>",
            ),
            (
                CLOSE,
                "<rate_limit sense=\"incr\">0.2</rate_limit></actuator>",
            ),
        ],
        RATE_PROPERTIES,
    ),
    prepared(
        "rate5",
        &[
            (RATE, "<rate_limit sense=\"decr\">0.1</rate_limit>"),
            (
                CLOSE,
                "<rate_limit sense=\"incr\">{p}/rate-limit-value</rate_limit></actuator>",
            ),
        ],
        RATE_PROPERTIES,
    ),
    prepared(
        "rate6",
        &[
            (
                RATE,
                "<rate_limit sense=\"decr\">{p}/rate-limit-value2</rate_limit>",
            ),
            (
                CLOSE,
                "<rate_limit sense=\"incr\">{p}/rate-limit-value</rate_limit></actuator>",
            ),
        ],
        RATE_PROPERTIES,
    ),
    // test_clipto, cumulatively.
    prepared("clip1", &[], &[]),
    prepared(
        "clip2",
        &[(CLIP_MIN, "<min>{p}/clip-min-value</min>")],
        CLIP_PROPERTIES,
    ),
    prepared(
        "clip3",
        &[(CLIP_MIN, "<min>-{p}/clip-max-value</min>")],
        CLIP_PROPERTIES,
    ),
    prepared(
        "clip4",
        &[
            (CLIP_MIN, "<min>-0.1</min>"),
            (CLIP_MAX, "<max>{p}/clip-max-value</max>"),
        ],
        CLIP_PROPERTIES,
    ),
    prepared(
        "clip5",
        &[
            (CLIP_MIN, "<min>-0.1</min>"),
            (CLIP_MAX, "<max>-{p}/clip-min-value</max>"),
        ],
        CLIP_PROPERTIES,
    ),
    prepared(
        "clip6",
        &[
            (CLIP_MIN, "<min>-{p}/clip-max-value</min>"),
            (CLIP_MAX, "<max>{p}/clip-max-value</max>"),
        ],
        CLIP_PROPERTIES,
    ),
    prepared(
        "cyclic1",
        &[
            ("<clipto>", "<clipto type=\"cyclic\">"),
            (CLIP_MIN, "<min>-3.141592653589793</min>"),
            (CLIP_MAX, "<max>3.141592653589793</max>"),
        ],
        CLIP_PROPERTIES,
    ),
    prepared(
        "cyclic2",
        &[
            ("<clipto>", "<clipto type=\"cyclic\">"),
            (CLIP_MIN, "<min>0.0</min>"),
            (CLIP_MAX, "<max>3.141592653589793</max>"),
        ],
        CLIP_PROPERTIES,
    ),
    prepared(
        "cyclic3",
        &[
            ("<clipto>", "<clipto type=\"cyclic\">"),
            (CLIP_MIN, "<min>-3.141592653589793</min>"),
            (CLIP_MAX, "<max>0.0</max>"),
        ],
        CLIP_PROPERTIES,
    ),
    // test_failhardover_without_clipto.
    prepared(
        "noclip",
        &[(
            "              <clipto>\n                <min> -0.350 </min>\n                <max>  0.260 </max>\n              </clipto>\n",
            "",
        )],
        &[],
    ),
];

fn fail(message: &str) -> OracleError {
    OracleError::Provenance(format!("aircraft/c172x/c172x.xml: {message}"))
}

/// The element `<actuator>` the test edits: the first one in
/// `<flight_control>` with a `<rate_limit>`.
fn rate_limited_actuator(text: &str) -> Result<&str, OracleError> {
    let fcs_at = text
        .find("<flight_control")
        .ok_or_else(|| fail("no <flight_control>"))?;
    let mut rest = &text[fcs_at..];
    while let Some(start) = rest.find("<actuator ") {
        let after = &rest[start..];
        let end = after
            .find(CLOSE)
            .ok_or_else(|| fail("unterminated <actuator>"))?
            + CLOSE.len();
        let element = &after[..end];
        if element.contains("<rate_limit") {
            // From the start of its line, so the indentation stays.
            let line_start = rest[..start].rfind('\n').map_or(0, |i| i + 1);
            return Ok(&rest[line_start..start + end]);
        }
        rest = &after[end..];
    }
    Err(fail("no actuator with a <rate_limit>"))
}

fn replace_once(text: &str, old: &str, new: &str) -> Result<String, OracleError> {
    if !text.contains(old) {
        return Err(fail(&format!("the actuator has no {old:?}")));
    }
    Ok(text.replacen(old, new, 1))
}

/// One configuration's actuator element.
fn configured(element: &str, config: &Config) -> Result<String, OracleError> {
    let prefix = format!("fcs/{}", config.label);
    let mut out = element.replace("fcs/left-aileron-", &format!("{prefix}/left-aileron-"));
    if config.prepared {
        out = replace_once(
            &out,
            "              <hysteresis_width> 0.005 </hysteresis_width>\n",
            "",
        )?;
        out = replace_once(
            &out,
            &format!("<input> {prefix}/left-aileron-control </input>"),
            &format!("<input>{prefix}/left-aileron-input</input>"),
        )?;
    }
    for (old, new) in config.edits {
        out = replace_once(&out, old, &new.replace("{p}", &prefix))?;
    }
    Ok(out)
}

/// `actuator_c172x.xml` from the upstream c172x text.
pub fn system(c172x: &str) -> Result<String, OracleError> {
    let element = rate_limited_actuator(c172x)?;
    let mut out = String::from(
        "<?xml version=\"1.0\"?>\n<!--\n  SPDX-License-Identifier: LGPL-2.1-or-later\n  actuator_c172x.xml: fcs/left-aileron-actuator of aircraft/c172x/c172x.xml\n  in each configuration JSBSim's tests/TestActuator.py edits it into, one\n  channel per configuration, its properties prefixed fcs/<label>/.\n  Regenerate with `oracle derive`.\n-->\n<system name=\"actuator_c172x\">\n",
    );
    for config in &CONFIGS {
        let prefix = format!("fcs/{}", config.label);
        if config.prepared {
            out.push_str(&format!(
                "  <property value=\"0.0\">{prefix}/left-aileron-input</property>\n"
            ));
        }
        for (name, value) in config.properties {
            match value {
                Some(v) => out.push_str(&format!(
                    "  <property value=\"{v}\">{prefix}/{name}</property>\n"
                )),
                None => out.push_str(&format!("  <property>{prefix}/{name}</property>\n")),
            }
        }
    }
    for config in &CONFIGS {
        out.push_str(&format!("  <channel name=\"{}\">\n", config.label));
        out.push_str(&configured(element, config)?);
        out.push_str("\n  </channel>\n");
    }
    out.push_str("</system>\n");
    Ok(out)
}
