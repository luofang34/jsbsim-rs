// SPDX-License-Identifier: LGPL-2.1-or-later
use std::path::Path;

use super::*;

fn row(property: &str, t_start: Option<f64>, t_end: Option<f64>) -> Tolerance {
    Tolerance {
        case: "ball".into(),
        property: property.into(),
        abs: 1.0,
        rel: 0.0,
        period: None,
        t_start,
        t_end,
        reason: "test".into(),
    }
}

#[test]
fn parses_rows_and_bounds() {
    let table = ToleranceTable::parse(
        r#"
        [[tolerance]]
        case = "ball"
        property = "x"
        abs = 0.5
        rel = 1e-3
        reason = "because"
        "#,
        Path::new("t.toml"),
    )
    .unwrap();
    let tol = table.lookup("ball", "x", 3.0).unwrap();
    assert_eq!(tol.allowed(-100.0), 0.5 + 0.1);
    assert!(table.lookup("ball", "y", 3.0).is_none());
    assert!(table.lookup("other", "x", 3.0).is_none());
}

#[test]
fn empty_reason_is_rejected() {
    let mut r = row("x", None, None);
    r.reason = "  ".into();
    let err = ToleranceTable::new(vec![r]).unwrap_err();
    assert_eq!(
        err.to_string(),
        "tolerance row 0 (ball x): reason must not be empty"
    );
}

#[test]
fn missing_reason_field_is_rejected() {
    let err = ToleranceTable::parse(
        "[[tolerance]]\ncase = \"b\"\nproperty = \"x\"\nabs = 0.0\nrel = 0.0\n",
        Path::new("t.toml"),
    )
    .unwrap_err();
    assert!(err.to_string().contains("reason"), "{err}");
}

#[test]
fn negative_or_nan_bounds_are_rejected() {
    for (abs, rel) in [(-1.0, 0.0), (0.0, f64::NAN), (f64::INFINITY, 0.0)] {
        let mut r = row("x", None, None);
        r.abs = abs;
        r.rel = rel;
        assert!(ToleranceTable::new(vec![r]).is_err(), "{abs} {rel}");
    }
}

#[test]
fn windows_are_half_open() {
    let table =
        ToleranceTable::new(vec![row("x", None, Some(10.0)), row("x", Some(10.0), None)]).unwrap();
    assert_eq!(table.lookup("ball", "x", 9.999).unwrap().t_end, Some(10.0));
    assert_eq!(table.lookup("ball", "x", 10.0).unwrap().t_start, Some(10.0));
}

#[test]
fn gaps_between_windows_have_no_tolerance() {
    let table = ToleranceTable::new(vec![row("x", Some(0.0), Some(1.0))]).unwrap();
    assert!(table.lookup("ball", "x", 1.0).is_none());
    assert!(table.lookup("ball", "x", -0.1).is_none());
}

#[test]
fn overlapping_windows_are_rejected() {
    let err = ToleranceTable::new(vec![row("x", None, Some(10.0)), row("x", Some(5.0), None)])
        .unwrap_err();
    assert!(err.to_string().contains("overlaps"), "{err}");
    // Same window on different properties is fine.
    ToleranceTable::new(vec![row("x", None, None), row("y", None, None)]).unwrap();
}

#[test]
fn inverted_window_is_rejected() {
    assert!(ToleranceTable::new(vec![row("x", Some(2.0), Some(1.0))]).is_err());
    assert!(ToleranceTable::new(vec![row("x", Some(1.0), Some(1.0))]).is_err());
    assert!(ToleranceTable::new(vec![row("x", Some(f64::NAN), None)]).is_err());
}

#[test]
fn unknown_fields_are_rejected() {
    let err = ToleranceTable::parse(
        "[[tolerance]]\ncase=\"b\"\nproperty=\"x\"\nabs=0.0\nrel=0.0\nreason=\"r\"\nwhy=\"\"\n",
        Path::new("t.toml"),
    )
    .unwrap_err();
    assert!(err.to_string().contains("why"), "{err}");
}

#[test]
fn periodic_rows_compare_wrapped_differences() {
    let mut r = row("phi", None, None);
    r.abs = 1e-9;
    r.period = Some(std::f64::consts::TAU);
    let pi = std::f64::consts::PI;
    assert!(r.agrees(pi - 1e-12, -pi + 1e-12), "across the branch cut");
    assert!(!r.agrees(0.0, 1e-3));
    assert!(r.difference(170.0f64.to_radians(), -170.0f64.to_radians()) < 0.35);
    r.period = Some(-1.0);
    assert!(ToleranceTable::new(vec![r]).is_err());
}
