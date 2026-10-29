// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

fn series(values: &[(f64, f64)]) -> Series {
    Series {
        columns: vec!["Time".into(), "x".into()],
        rows: values.iter().map(|&(t, x)| vec![t, x]).collect(),
    }
}

fn max_of(reference: &[(f64, f64)], candidate: &[(f64, f64)]) -> Max {
    let found = maxima(
        &series(reference),
        &series(candidate),
        f64::INFINITY,
        |_| None,
    );
    found[0].1
}

#[test]
fn equal_infinities_are_no_difference() {
    let inf = f64::INFINITY;
    let m = max_of(
        &[(0.0, inf), (0.2, inf), (0.4, 11.0)],
        &[(0.0, inf), (0.2, inf), (0.4, 11.0 + 4e-11)],
    );
    assert!((m.abs - 4e-11).abs() < 1e-15, "{m:?}");
    assert_eq!(m.time, 0.4);
    assert_eq!(m.mismatches, 0);
}

#[test]
fn an_infinity_against_a_finite_value_is_a_mismatch() {
    let m = max_of(
        &[(0.0, f64::INFINITY), (0.2, 1.0)],
        &[(0.0, 1.0), (0.2, 1.5)],
    );
    assert_eq!((m.abs, m.time), (0.5, 0.2));
    assert_eq!((m.mismatches, m.first_mismatch), (1, Some(0.0)));
}

#[test]
fn opposite_infinities_are_a_mismatch() {
    let m = max_of(&[(0.0, f64::INFINITY)], &[(0.0, f64::NEG_INFINITY)]);
    assert_eq!(m.abs, 0.0);
    assert_eq!(m.mismatches, 1);
}

#[test]
fn nan_on_both_sides_matches_as_the_comparator_says() {
    let tolerance = crate::Tolerance {
        case: "c".into(),
        property: "x".into(),
        abs: 0.0,
        rel: 0.0,
        period: None,
        t_start: None,
        t_end: None,
        reason: "test".into(),
    };
    assert!(tolerance.agrees(f64::NAN, f64::NAN));
    let m = max_of(
        &[(0.0, f64::NAN), (0.2, 2.0)],
        &[(0.0, f64::NAN), (0.2, 2.25)],
    );
    assert_eq!((m.abs, m.mismatches), (0.25, 0));
}

#[test]
fn nan_on_one_side_is_a_mismatch_and_never_a_maximum() {
    let m = max_of(
        &[(0.0, 1.0), (0.2, f64::NAN), (0.4, 3.0)],
        &[(0.0, 1.25), (0.2, 2.0), (0.4, 3.5)],
    );
    assert!(!m.abs.is_nan() && !m.rel.is_nan());
    assert_eq!((m.abs, m.time), (0.5, 0.4));
    assert_eq!((m.mismatches, m.first_mismatch), (1, Some(0.2)));
}
