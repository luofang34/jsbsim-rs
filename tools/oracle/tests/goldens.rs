// SPDX-License-Identifier: LGPL-2.1-or-later
//! Integrity of the committed goldens and tolerance table.

use std::path::{Path, PathBuf};

use oracle::manifest::{self, REFERENCE_FILE};
use oracle::{Manifest, Series, ToleranceTable, compare};

#[cfg(test)]
fn oracle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

#[cfg(test)]
fn cases() -> Vec<String> {
    manifest::list_cases_blocking(&oracle_dir().join("goldens")).unwrap()
}

#[cfg(test)]
fn load(case: &str) -> (Manifest, Series) {
    let dir = oracle_dir().join("goldens").join(case);
    (
        Manifest::read_blocking(&dir).unwrap(),
        Series::read_blocking(&dir.join(REFERENCE_FILE)).unwrap(),
    )
}

#[cfg(test)]
fn table() -> ToleranceTable {
    ToleranceTable::read_blocking(&oracle_dir().join("tolerances.toml")).unwrap()
}

#[test]
fn ball_case_is_committed() {
    assert!(cases().contains(&"ball".to_owned()));
}

#[test]
fn every_golden_matches_its_manifest() {
    for case in cases() {
        let (manifest, series) = load(&case);
        let provenance = manifest.provenance.as_ref().expect(&case);
        assert_eq!(provenance.upstream_commit.len(), 40, "{case}");
        assert!(
            provenance
                .upstream_commit
                .bytes()
                .all(|b| b.is_ascii_hexdigit()),
            "{case}"
        );
        assert_eq!(series.columns, manifest.expected_columns(), "{case}");
        assert_eq!(series.rows.len(), provenance.rows, "{case}");
    }
}

#[test]
fn samples_are_spaced_by_the_frame_interval() {
    for case in cases() {
        let (manifest, series) = load(&case);
        let provenance = manifest.provenance.unwrap();
        let interval = provenance.dt * provenance.every_n_frames as f64;
        for (i, row) in series.rows.iter().enumerate() {
            let expected = i as f64 * interval;
            assert!(
                (row[0] - expected).abs() < 1e-6,
                "{case} row {i}: {}",
                row[0]
            );
        }
    }
}

#[test]
fn ball_samples_the_whole_script_at_the_requested_rate() {
    let (manifest, series) = load("ball");
    let provenance = manifest.provenance.unwrap();
    assert_eq!(provenance.dt, 0.01);
    // 0.1 Hz at dt = 0.01 s is every 1000 frames; ball.xml ends at 10800 s.
    assert_eq!(provenance.every_n_frames, 1000);
    assert_eq!(series.rows.len(), 10800 / 10 + 1);
}

#[test]
fn every_golden_agrees_with_itself_under_the_table() {
    let table = table();
    for case in cases() {
        let (_, series) = load(&case);
        let outcome = compare(&case, &series, &series, &table).unwrap();
        assert_eq!(outcome.divergence, None, "{case}");
        assert_eq!(outcome.rows_compared, series.rows.len());
    }
}

#[test]
fn every_tolerance_row_belongs_to_a_logged_column() {
    let cases = cases();
    for row in table().rows() {
        assert!(cases.contains(&row.case), "orphan case {}", row.case);
        let (manifest, _) = load(&row.case);
        assert!(
            manifest.expected_columns().contains(&row.property),
            "{} does not log {}",
            row.case,
            row.property
        );
    }
}
