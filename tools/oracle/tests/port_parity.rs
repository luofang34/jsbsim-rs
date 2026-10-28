// SPDX-License-Identifier: LGPL-2.1-or-later
//! The Rust port against the committed C++ references.

use std::path::{Path, PathBuf};

use oracle::ToleranceTable;
use oracle::manifest::{self, CaseSpec, INPUT_FILE, Manifest, TRANSCRIPT_FILE};
use oracle::transcript::compare_transcript_declaring;

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
    let context = oracle::probe::ProbeContext {
        input_dir: dir.clone(),
        jsbsim_root: None,
    };
    let candidate = oracle::probe::run(&spec.probe, &input, &context).unwrap();
    let table = ToleranceTable::read_blocking(&goldens().join("../tolerances.toml")).unwrap();
    let outcome =
        compare_transcript_declaring(case, &reference, &candidate, &table, &spec.unsupported)
            .unwrap();
    if let Some(divergence) = outcome.divergence {
        panic!("{case} {divergence}");
    }
}

#[test]
fn every_transcript_case_matches_its_cpp_reference() {
    let cases = manifest::list_cases_blocking(&goldens()).unwrap();
    let mut checked = 0;
    for case in cases {
        let manifest = Manifest::read_blocking(&goldens().join(&case)).unwrap();
        if matches!(manifest.case, CaseSpec::Transcript(_)) {
            transcript_parity(&case);
            checked += 1;
        }
    }
    assert!(checked >= 10, "only {checked} transcript cases found");
}
