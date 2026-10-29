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

#[test]
fn bench_takes_case_names() {
    let matches = command()
        .try_get_matches_from([
            "oracle",
            "bench",
            "--jsbsim-root",
            "/j",
            "ball",
            "mk82_script",
        ])
        .unwrap();
    let (name, sub) = matches.subcommand().unwrap();
    assert_eq!(name, "bench");
    assert_eq!(case_names(sub), ["ball", "mk82_script"]);
}

#[test]
fn every_subcommand_parses_without_a_case_argument() {
    for sub in ["build", "generate", "check-drift", "check-xml"] {
        let matches = command()
            .try_get_matches_from(["oracle", sub, "--jsbsim-root", "/nonexistent"])
            .unwrap();
        let (_, sub_matches) = matches.subcommand().unwrap();
        let paths = Paths {
            repo_root: PathBuf::from("/nonexistent"),
            jsbsim_root: PathBuf::from("/nonexistent"),
            build: crate::upstream::Build::Gate,
        };
        // Must not panic on subcommands that define no case argument.
        selected_cases(&paths, sub_matches).ok();
    }
}
