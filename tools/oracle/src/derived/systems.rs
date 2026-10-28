// SPDX-License-Identifier: LGPL-2.1-or-later
//! Systems derived from upstream `tests/function.xml`: without the
//! `<random>` and `<urandom>` components the port does not mirror, and
//! that again with its table given an unknown type, as
//! `TestFunctions.test_table_exception` edits it.

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
