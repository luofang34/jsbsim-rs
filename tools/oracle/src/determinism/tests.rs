// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn every_listed_case_is_a_golden_case() {
    let root = repo_root();
    let cases = read_cases_blocking(&root).unwrap();
    assert!(!cases.is_empty());
    let mut seen = std::collections::BTreeSet::new();
    for case in &cases {
        assert!(seen.insert(case), "{case} is listed twice");
        let dir = manifest::case_dir(&root.join("oracle").join("goldens"), case);
        Manifest::read_blocking(&dir).unwrap_or_else(|e| panic!("{case}: {e}"));
    }
}

#[test]
fn the_list_covers_the_flight_cases_and_the_noisy_c172x() {
    let cases = read_cases_blocking(&repo_root()).unwrap();
    for case in ["ball", "mk82", "c1722", "737_cruise", "f16_test"] {
        assert!(cases.iter().any(|c| c == case), "{case} is not listed");
    }
}

fn series(rows: Vec<Vec<f64>>) -> Series {
    Series {
        columns: vec!["Time".to_owned(), "a".to_owned(), "b".to_owned()],
        rows,
    }
}

#[test]
fn bits_round_trip_and_agree_with_themselves() {
    let text = bits_text(&series(vec![
        vec![0.0, -0.0, f64::MIN_POSITIVE],
        vec![0.5, 1.0, -2.5],
    ]));
    assert_eq!(
        text,
        "Time,a,b\n0000000000000000,8000000000000000,0010000000000000\n\
         3fe0000000000000,3ff0000000000000,c004000000000000\n"
    );
    let outcome = compare_records("t", true, &text, &text).unwrap();
    assert_eq!(
        (outcome.rows, outcome.values, outcome.mismatch),
        (2, 6, None)
    );
}

#[test]
fn the_first_differing_bit_is_reported_with_its_time_and_property() {
    let native = bits_text(&series(vec![vec![0.0, 1.0, 2.0], vec![0.5, 1.0, 2.0]]));
    let wasm = bits_text(&series(vec![
        vec![0.0, 1.0, 2.0],
        vec![0.5, 1.0, f64::from_bits(2.0_f64.to_bits() + 1)],
    ]));
    let outcome = compare_records("t", true, &native, &wasm).unwrap();
    assert_eq!(outcome.rows, 1);
    assert_eq!(
        outcome.mismatch,
        Some(Mismatch::Value {
            time: 0.5,
            property: "b".to_owned(),
            native: 2.0_f64.to_bits(),
            wasm: 2.0_f64.to_bits() + 1,
        })
    );
}

#[test]
fn a_negative_zero_or_another_nan_is_a_mismatch() {
    let native = bits_text(&series(vec![vec![0.0, 0.0, f64::NAN]]));
    let signed = bits_text(&series(vec![vec![0.0, -0.0, f64::NAN]]));
    let other_nan = bits_text(&series(vec![vec![0.0, 0.0, -f64::NAN]]));
    for wasm in [signed, other_nan] {
        let outcome = compare_records("t", true, &native, &wasm).unwrap();
        assert!(
            matches!(outcome.mismatch, Some(Mismatch::Value { .. })),
            "{wasm}"
        );
    }
}

#[test]
fn missing_rows_and_other_headers_are_mismatches() {
    let long = bits_text(&series(vec![vec![0.0, 1.0, 2.0], vec![0.5, 1.0, 2.0]]));
    let short = bits_text(&series(vec![vec![0.0, 1.0, 2.0]]));
    let outcome = compare_records("t", true, &long, &short).unwrap();
    assert_eq!(
        outcome.mismatch,
        Some(Mismatch::Length { native: 3, wasm: 2 })
    );
    let renamed = long.replacen('b', "c", 1);
    let outcome = compare_records("t", true, &long, &renamed).unwrap();
    assert!(matches!(outcome.mismatch, Some(Mismatch::Header { .. })));
}

#[test]
fn transcripts_compare_line_by_line() {
    let same = compare_records("t", false, "x\ny\n", "x\ny\n").unwrap();
    assert_eq!((same.rows, same.mismatch), (2, None));
    let changed = compare_records("t", false, "x\ny\n", "x\nz\n").unwrap();
    assert_eq!(
        changed.mismatch,
        Some(Mismatch::Line {
            line: 2,
            native: "y".to_owned(),
            wasm: "z".to_owned(),
        })
    );
    let cut = compare_records("t", false, "x\ny\n", "x\n").unwrap();
    assert_eq!(cut.mismatch, Some(Mismatch::Length { native: 2, wasm: 1 }));
}

#[test]
fn a_bad_cell_is_an_error() {
    assert!(compare_records("t", true, "Time\nzz\n", "Time\nzz\n").is_err());
}

#[test]
fn a_transcript_case_records_the_same_text_twice() {
    let root = repo_root();
    let first = record_blocking(&root, "random").unwrap();
    assert!(matches!(first, Record::Transcript(_)));
    assert_eq!(first, record_blocking(&root, "random").unwrap());
}
