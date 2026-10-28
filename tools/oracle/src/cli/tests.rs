// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

#[test]
fn command_definition_is_valid() {
    command().debug_assert();
}

#[test]
fn subcommand_is_required() {
    assert!(matches!(run(["oracle"]), Err(OracleError::Usage(_))));
}

#[test]
fn compare_requires_a_case() {
    assert!(matches!(
        run(["oracle", "compare", "a.csv", "b.csv"]),
        Err(OracleError::Usage(_))
    ));
}
