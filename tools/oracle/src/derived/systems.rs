// SPDX-License-Identifier: LGPL-2.1-or-later
//! Systems derived from upstream `tests/function.xml`: without the
//! `<random>` and `<urandom>` components the port does not mirror, and
//! that again with its table given an unknown type, as
//! `TestFunctions.test_table_exception` edits it; and the waypoint channel
//! of `systems/GNCUtilities.xml` ([`gnc_waypoint`]).

use crate::OracleError;

/// The components of `tests/function.xml` dropped from the derived
/// systems: JSBSim's random generators are not mirrored.
const RANDOM: [&str; 3] = ["test/random", "test/random2", "test/urandom"];

fn header(name: &str, what: &str) -> String {
    format!(
        "<!--\n  SPDX-License-Identifier: LGPL-2.1-or-later\n  {name}: tests/function.xml {what}\n  Regenerate with `oracle derive`.\n-->\n"
    )
}

fn without_random_components(text: &str) -> Result<String, OracleError> {
    let mut out = text.to_owned();
    for name in RANDOM {
        let open = format!("    <fcs_function name=\"{name}\">\n");
        let start = out
            .find(&open)
            .ok_or_else(|| OracleError::Provenance(format!("tests/function.xml: no {name}")))?;
        let end = out[start..]
            .find("</fcs_function>\n")
            .map(|e| start + e + "</fcs_function>\n".len())
            .ok_or_else(|| OracleError::Provenance(format!("tests/function.xml: {name} open")))?;
        out.replace_range(start..end, "");
    }
    Ok(out)
}

/// `function_no_random.xml`: the upstream text without its random
/// components.
pub fn no_random(text: &str) -> Result<String, OracleError> {
    let body = without_random_components(text)?;
    Ok(header(
        "function_no_random.xml",
        "without the test/random, test/random2 and test/urandom\n  components, whose <random> and <urandom> the port does not mirror.",
    ) + &body)
}

/// `function_bad_table.xml`: `function_no_random.xml` with the first
/// `channel/fcs_function/function/table` typed `qsdfghjkl`, as
/// `TestFunctions.test_table_exception` edits the upstream file.
pub fn bad_table(text: &str) -> Result<String, OracleError> {
    let body = without_random_components(text)?;
    let marker = "      <function>\n        <table>";
    let at = body.find(marker).ok_or_else(|| {
        OracleError::Provenance("tests/function.xml: no function table".to_owned())
    })?;
    let mut body = body;
    body.replace_range(
        at..at + marker.len(),
        "      <function>\n        <table type=\"qsdfghjkl\">",
    );
    Ok(header(
        "function_bad_table.xml",
        "without its random components (see\n  function_no_random.xml) and with its first function table typed\n  \"qsdfghjkl\", as TestFunctions.test_table_exception edits it.",
    ) + &body)
}

/// `gnc_waypoint.xml`: upstream `systems/GNCUtilities.xml`, which
/// `TestWaypoint.py` runs through c310, without its "Included Angle to
/// Heading" channel, so that the fcs-waypoint case reads only the
/// waypoint channel the test reads; fcs-angles-gnc runs the whole file.
pub fn gnc_waypoint(text: &str) -> Result<String, OracleError> {
    let fail =
        |message: &str| OracleError::Provenance(format!("systems/GNCUtilities.xml: {message}"));
    let declaration = "<?xml version=\"1.0\"?>\n";
    let body = text
        .strip_prefix(declaration)
        .ok_or_else(|| fail("no XML declaration"))?;
    let open = "  <channel name=\"Included Angle to Heading\">\n";
    let close = "  </channel>\n";
    let start = body.find(open).ok_or_else(|| fail("no angle channel"))?;
    let end = body[start..]
        .find(close)
        .map(|e| start + e + close.len())
        .ok_or_else(|| fail("angle channel open"))?;
    let mut out = String::with_capacity(text.len() + 512);
    out.push_str(declaration);
    out.push_str(
        "<!--\n  SPDX-License-Identifier: LGPL-2.1-or-later\n  gnc_waypoint.xml: systems/GNCUtilities.xml without its \"Included Angle\n  to Heading\" channel, so that it holds only the waypoint channel\n  TestWaypoint.py reads. Regenerate with `oracle derive`.\n-->\n",
    );
    out.push_str(&body[..start]);
    out.push_str(&body[end..]);
    Ok(out)
}
