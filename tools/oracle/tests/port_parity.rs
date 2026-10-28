// SPDX-License-Identifier: LGPL-2.1-or-later
//! The Rust port against the committed C++ references.

use std::path::{Path, PathBuf};

use oracle::cases::first_line_difference;
use oracle::manifest::{self, CaseSpec, INPUT_FILE, Manifest, TRANSCRIPT_FILE};

#[cfg(test)]
fn goldens() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/goldens")
}

#[cfg(test)]
fn transcript_parity(case: &str) {
    let dir = goldens().join(case);
    let CaseSpec::Transcript(spec) = Manifest::read_blocking(&dir).unwrap().case else {
        panic!("{case} is not a transcript case");
    };
    let input = std::fs::read_to_string(dir.join(INPUT_FILE)).unwrap();
    let reference = std::fs::read_to_string(dir.join(TRANSCRIPT_FILE)).unwrap();
    let candidate = oracle::probe::run(&spec.probe, &input).unwrap();
    if let Some((line, expected, got)) = first_line_difference(&reference, &candidate) {
        panic!("{case} line {line}:\n  C++:  {expected}\n  Rust: {got}");
    }
}

#[test]
fn every_transcript_case_has_a_parity_test() {
    let tested = ["properties"];
    let cases = manifest::list_cases_blocking(&goldens()).unwrap();
    for case in cases {
        let manifest = Manifest::read_blocking(&goldens().join(&case)).unwrap();
        if matches!(manifest.case, CaseSpec::Transcript(_)) {
            assert!(tested.contains(&case.as_str()), "no parity test for {case}");
        }
    }
}

#[test]
fn properties() {
    transcript_parity("properties");
}
