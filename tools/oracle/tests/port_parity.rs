// SPDX-License-Identifier: LGPL-2.1-or-later
//! The Rust port against the committed C++ references.

use std::path::{Path, PathBuf};

use oracle::ToleranceTable;
use oracle::manifest::{self, CaseSpec, INPUT_FILE, Manifest, TRANSCRIPT_FILE};
use oracle::margin::Margins;
use oracle::transcript::compare_transcript_declaring;

#[cfg(test)]
fn goldens() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/goldens")
}

#[cfg(test)]
fn transcript_parity(case: &str, table: &ToleranceTable, margins: &mut Margins) {
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
    margins
        .observe_transcript(case, &reference, &candidate, table, &spec.unsupported)
        .unwrap();
    let outcome =
        compare_transcript_declaring(case, &reference, &candidate, table, &spec.unsupported)
            .unwrap();
    if let Some(divergence) = outcome.divergence {
        panic!("{case} {divergence}");
    }
}

/// Also checks the margins the transcript cases' rows claim, as `oracle
/// port` does for every case.
#[test]
fn every_transcript_case_matches_its_cpp_reference() {
    let table = ToleranceTable::read_blocking(&goldens().join("../tolerances.toml")).unwrap();
    let mut margins = Margins::new(&table);
    let mut checked = Vec::new();
    for case in manifest::list_cases_blocking(&goldens()).unwrap() {
        let manifest = Manifest::read_blocking(&goldens().join(&case)).unwrap();
        if matches!(manifest.case, CaseSpec::Transcript(_)) {
            transcript_parity(&case, &table, &mut margins);
            checked.push(case);
        }
    }
    assert!(
        checked.len() >= 10,
        "only {} transcript cases found",
        checked.len()
    );
    let refuted: Vec<String> = margins
        .refuted(&table, &checked)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(refuted.is_empty(), "{}", refuted.join("\n"));
}
