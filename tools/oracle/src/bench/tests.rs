// SPDX-License-Identifier: LGPL-2.1-or-later
use std::collections::HashSet;

use super::*;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn committed_cases() -> BenchFile {
    BenchFile::read_blocking(&repo_root().join(CASES_FILE)).unwrap()
}

#[test]
fn parses_the_driver_lines_among_jsbsim_messages() {
    let stdout = "\nIn file ball.xml: line 134\nNo direction element\n\
                  bench frames=2000 ns=35430084\n  The aerodynamic moment axis\n\
                  bench frames=2000 ns=37625083\n";
    assert_eq!(
        parse_bench_output(stdout, 2000).unwrap(),
        vec![35_430_084, 37_625_083]
    );
}

#[test]
fn rejects_missing_or_malformed_driver_lines() {
    assert!(parse_bench_output("script was not loaded\n", 10).is_err());
    assert!(parse_bench_output("bench frames=10 ns=abc\n", 10).is_err());
    assert!(parse_bench_output("bench frames=10\n", 10).is_err());
    assert!(parse_bench_output("bench frames=10 ns=5 extra\n", 10).is_err());
    assert!(parse_bench_output("bench frames=11 ns=5\n", 10).is_err());
}

#[test]
fn median_of_an_odd_count_is_the_middle_value() {
    let s = Summary::of(&[30.0, 10.0, 20.0]).unwrap();
    assert_eq!(
        s,
        Summary {
            median: 20.0,
            min: 10.0,
            max: 30.0
        }
    );
}

#[test]
fn median_of_an_even_count_is_the_mean_of_the_middle_pair() {
    let s = Summary::of(&[4.0, 1.0, 3.0, 2.0]).unwrap();
    assert_eq!(s.median, 2.5);
    assert_eq!((s.min, s.max), (1.0, 4.0));
    assert_eq!(Summary::of(&[7.0]).unwrap().median, 7.0);
    assert_eq!(Summary::of(&[]), None);
}

#[test]
fn ratio_fps_and_per_frame() {
    assert_eq!(ratio(1500.0, 1000.0), 1.5);
    assert_eq!(fps(2000.0), 500_000.0);
    assert_eq!(per_frame(3_000_000, 2000), 1500.0);
}

#[test]
fn a_row_carries_the_ratio_and_both_rates() {
    let row = format_row(&CaseResult {
        name: "ball".to_owned(),
        frames: 100,
        cpp: Summary::of(&[1000.0]).unwrap(),
        rust: Summary::of(&[800.0, 900.0]).unwrap(),
    });
    assert!(row.starts_with("ball "), "{row}");
    assert!(row.contains("0.850"), "{row}");
    assert!(row.contains("1176471"), "{row}");
    assert!(row.contains("1000000"), "{row}");
}

#[test]
fn unknown_case_names_are_refused() {
    let file = committed_cases();
    assert_eq!(file.select(&[]).unwrap(), file.cases);
    assert!(matches!(
        file.select(&["nope".to_owned()]),
        Err(OracleError::Bench { .. })
    ));
    let picked = file.select(&["cannonball".to_owned()]).unwrap();
    assert_eq!(picked.len(), 1);
}

#[test]
fn committed_cases_parse_and_name_existing_inputs() {
    let file = committed_cases();
    assert!(!file.cases.is_empty());
    let root = rust_root(&repo_root());
    let mut names = HashSet::new();
    for case in &file.cases {
        assert!(names.insert(&case.name), "duplicate case {}", case.name);
        assert!(case.frames > 0 && case.repeats > 0, "{}", case.name);
        assert!(root.join(&case.script).is_file(), "{}", case.script);
        let aircraft = case.aircraft_path.as_deref().unwrap_or("aircraft");
        assert!(root.join(aircraft).is_dir(), "{aircraft}");
    }
}

#[test]
fn unknown_fields_are_refused() {
    let text = "[[case]]\nname = \"a\"\nscript = \"s\"\nwarm_up = 1\n\
                frames = 1\nrepeats = 1\nframe = 2\n";
    assert!(BenchFile::parse(text, Path::new("bench.toml")).is_err());
}

/// The frame budget of every case stays clear of its script's end and of
/// any touchdown, so a bench run times the same work on every repeat.
#[test]
fn every_case_runs_its_frames_in_the_port() {
    let root = rust_root(&repo_root());
    for case in &committed_cases().cases {
        rust::time(&root, case).unwrap();
    }
}
