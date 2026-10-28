// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

fn table(abs: f64) -> ToleranceTable {
    ToleranceTable::new(
        ["Time", "x", "y"]
            .into_iter()
            .map(|property| Tolerance {
                case: "c".into(),
                property: property.into(),
                abs,
                rel: 0.0,
                period: None,
                t_start: None,
                t_end: None,
                reason: "test".into(),
            })
            .collect(),
    )
    .unwrap()
}

fn series(rows: &[[f64; 3]]) -> Series {
    Series {
        columns: vec!["Time".into(), "x".into(), "y".into()],
        rows: rows.iter().map(|r| r.to_vec()).collect(),
    }
}

fn value_divergence(outcome: &Outcome) -> (f64, &str) {
    match outcome.divergence.as_ref().unwrap() {
        Divergence::Value { time, property, .. } => (*time, property.as_str()),
        other @ Divergence::RowCount { .. } => panic!("unexpected {other}"),
    }
}

#[test]
fn identical_series_agree() {
    let s = series(&[[0.0, 1.0, 2.0], [1.0, 3.0, 4.0]]);
    let outcome = compare("c", &s, &s, &table(0.0)).unwrap();
    assert_eq!(outcome.divergence, None);
    assert_eq!(outcome.rows_compared, 2);
    assert_eq!(outcome.columns, 3);
}

#[test]
fn difference_within_bound_agrees_and_beyond_diverges() {
    let reference = series(&[[0.0, 1.0, 2.0]]);
    let inside = series(&[[0.0, 1.5, 2.0]]);
    assert_eq!(
        compare("c", &reference, &inside, &table(0.5))
            .unwrap()
            .divergence,
        None
    );
    let outside = series(&[[0.0, 1.0, 2.6]]);
    let outcome = compare("c", &reference, &outside, &table(0.5)).unwrap();
    assert_eq!(value_divergence(&outcome), (0.0, "y"));
}

#[test]
fn reports_earliest_divergence_first() {
    let reference = series(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
    let candidate = series(&[[0.0, 0.0, 0.0], [1.0, 0.0, 9.0], [2.0, 9.0, 0.0]]);
    let outcome = compare("c", &reference, &candidate, &table(0.1)).unwrap();
    assert_eq!(value_divergence(&outcome), (1.0, "y"));
    assert_eq!(outcome.rows_compared, 2);
}

#[test]
fn time_grid_mismatch_is_a_divergence() {
    let reference = series(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]]);
    let candidate = series(&[[0.0, 0.0, 0.0], [1.5, 0.0, 0.0]]);
    let outcome = compare("c", &reference, &candidate, &table(0.1)).unwrap();
    assert_eq!(value_divergence(&outcome), (1.0, "Time"));
}

#[test]
fn nan_on_one_side_diverges() {
    let reference = series(&[[0.0, 1.0, 2.0]]);
    let candidate = series(&[[0.0, f64::NAN, 2.0]]);
    let outcome = compare("c", &reference, &candidate, &table(1e9)).unwrap();
    assert_eq!(value_divergence(&outcome), (0.0, "x"));
    let outcome = compare("c", &candidate, &reference, &table(1e9)).unwrap();
    assert_eq!(value_divergence(&outcome), (0.0, "x"));
}

#[test]
fn matching_nan_and_infinity_agree() {
    let s = series(&[[0.0, f64::NAN, f64::INFINITY]]);
    assert_eq!(compare("c", &s, &s, &table(0.0)).unwrap().divergence, None);
}

#[test]
fn missing_rows_are_reported_at_first_unmatched_time() {
    let reference = series(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
    let candidate = series(&[[0.0, 0.0, 0.0]]);
    let outcome = compare("c", &reference, &candidate, &table(0.0)).unwrap();
    assert_eq!(
        outcome.divergence,
        Some(Divergence::RowCount {
            time: 1.0,
            reference_rows: 3,
            candidate_rows: 1
        })
    );
}

#[test]
fn column_mismatch_is_an_error() {
    let reference = series(&[[0.0, 0.0, 0.0]]);
    let mut candidate = reference.clone();
    candidate.columns[2] = "z".into();
    assert!(matches!(
        compare("c", &reference, &candidate, &table(0.0)),
        Err(OracleError::ColumnMismatch { .. })
    ));
}

#[test]
fn unlisted_property_is_an_error_not_a_pass() {
    let s = series(&[[0.0, 0.0, 0.0]]);
    let err = compare("other-case", &s, &s, &table(0.0)).unwrap_err();
    assert_eq!(
        err.to_string(),
        "case other-case: no tolerance row covers Time at t=0"
    );
}

#[test]
fn display_names_time_property_values_and_bound() {
    let reference = series(&[[0.0, 1.0, 2.0]]);
    let candidate = series(&[[0.0, 1.0, 3.0]]);
    let outcome = compare("c", &reference, &candidate, &table(0.25)).unwrap();
    assert_eq!(
        outcome.divergence.unwrap().to_string(),
        "t=0 y: reference=2.0 candidate=3.0 |diff|=1.0 allowed=0.25 (abs=0.25 rel=0.0: test)"
    );
}
