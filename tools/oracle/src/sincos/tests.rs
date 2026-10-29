// SPDX-License-Identifier: LGPL-2.1-or-later
use std::collections::BTreeSet;
use std::path::Path;

use super::*;

fn goldens() -> Vec<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/goldens");
    manifest::list_cases_blocking(&dir).unwrap()
}

#[test]
fn every_listed_and_excepted_case_is_a_golden_named_once() {
    let goldens = goldens();
    let mut seen = BTreeSet::new();
    let excepted = EXCEPTIONS.iter().map(|e| e.case);
    for case in CASES.iter().copied().chain(excepted) {
        assert!(goldens.iter().any(|g| g == case), "{case} is not a golden");
        assert!(seen.insert(case), "{case} is listed twice");
    }
    assert!(!CASES.is_empty());
}

#[test]
fn every_golden_is_listed_or_excepted() {
    // A golden in neither list is one check-sincos never runs.
    let mut listed: Vec<&str> = CASES.to_vec();
    listed.extend(EXCEPTIONS.iter().map(|e| e.case));
    listed.sort_unstable();
    let goldens = goldens();
    let unlisted: Vec<&String> = goldens
        .iter()
        .filter(|g| listed.binary_search(&g.as_str()).is_err())
        .collect();
    assert!(
        unlisted.is_empty(),
        "goldens in neither CASES nor EXCEPTIONS: {unlisted:?}"
    );
}

#[test]
fn cases_are_sorted_and_named_once() {
    for pair in CASES.windows(2) {
        assert!(pair[0] < pair[1], "{} is not before {}", pair[0], pair[1]);
    }
}

#[test]
fn every_exception_states_its_reason() {
    for exception in EXCEPTIONS {
        assert!(!exception.reason.trim().is_empty(), "{}", exception.case);
    }
}

fn series(rows: Vec<Vec<f64>>) -> Series {
    Series {
        columns: vec!["Time".to_owned(), "x".to_owned()],
        rows,
    }
}

#[test]
fn only_identical_bits_agree() {
    let reference = series(vec![vec![0.0, 1.0], vec![0.5, f64::NAN]]);
    assert_eq!(first_difference(&reference, &reference.clone()), None);
    let one_ulp = series(vec![vec![0.0, f64::from_bits(1.0f64.to_bits() + 1)]]);
    let difference = first_difference(&reference, &one_ulp).unwrap();
    assert!(difference.starts_with("t=0 x:"), "{difference}");
    let zero = series(vec![vec![0.0, 0.0]]);
    let negative_zero = series(vec![vec![0.0, -0.0]]);
    assert!(first_difference(&zero, &negative_zero).is_some());
}

#[test]
fn a_short_or_differently_shaped_candidate_differs() {
    let reference = series(vec![vec![0.0, 1.0], vec![0.5, 2.0]]);
    let short = series(vec![vec![0.0, 1.0]]);
    let difference = first_difference(&reference, &short).unwrap();
    assert!(difference.contains("2 rows"), "{difference}");
    let mut renamed = reference.clone();
    renamed.columns[1] = "y".to_owned();
    assert!(first_difference(&reference, &renamed).is_some());
}

#[test]
fn the_libm_crate_is_told_apart_from_the_platform_libm() {
    // `cargo test` links the libm crate, not tools/libm-platform.
    assert!(!platform_libm_linked());
}
