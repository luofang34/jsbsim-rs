// SPDX-License-Identifier: LGPL-2.1-or-later
//! Systems derived from upstream ones: `tests/function.xml` with its
//! table given an unknown type, as `TestFunctions.test_table_exception`
//! edits it; and the waypoint channel of `systems/GNCUtilities.xml`
//! ([`gnc_waypoint`]).

use crate::OracleError;

fn header(name: &str, what: &str) -> String {
    format!(
        "<!--\n  SPDX-License-Identifier: LGPL-2.1-or-later\n  {name}: tests/function.xml {what}\n  Regenerate with `oracle derive`.\n-->\n"
    )
}

/// `function_bad_table.xml`: `tests/function.xml` with the first
/// `channel/fcs_function/function/table` typed `qsdfghjkl`, as
/// `TestFunctions.test_table_exception` edits it.
pub fn bad_table(text: &str) -> Result<String, OracleError> {
    let body = text.to_owned();
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
        "with its first function table typed \"qsdfghjkl\", as\n  TestFunctions.test_table_exception edits it.",
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
