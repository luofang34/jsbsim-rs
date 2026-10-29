// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

fn row(property: &str, abs: f64, rel: f64, reason: &str) -> Tolerance {
    Tolerance {
        case: "c".into(),
        property: property.into(),
        abs,
        rel,
        period: None,
        t_start: None,
        t_end: None,
        reason: reason.into(),
    }
}

fn series(values: &[f64]) -> Series {
    Series {
        columns: vec!["Time".into(), "x".into()],
        rows: values
            .iter()
            .enumerate()
            .map(|(i, &x)| vec![i as f64, x])
            .collect(),
    }
}

/// The refutations of a one-row table bounding `x` with `abs`, when the
/// largest difference observed is `diff`.
fn refuted(abs: f64, reason: &str, diff: f64) -> Vec<Refutation> {
    let table = ToleranceTable::new(vec![
        row("x", abs, 0.0, reason),
        row("*", 0.0, 0.0, "Time is exact."),
    ])
    .unwrap();
    let mut margins = Margins::new(&table);
    margins
        .observe_series(
            "c",
            &series(&[1.0, 2.0, 3.0]),
            &series(&[1.0, 2.0 + diff / 2.0, 3.0 + diff]),
            &table,
        )
        .unwrap();
    margins.refuted(&table, &["c".to_owned()])
}

const TEN: &str = "observed max 1.0e-3 at t=2 s; bound 10x that.";

#[test]
fn ten_times_within_its_range_passes() {
    assert_eq!(refuted(0.01, TEN, 1e-3), vec![]);
    assert_eq!(refuted(0.02, TEN, 1.01e-3), vec![]);
    assert_eq!(refuted(0.005, TEN, 0.0005), vec![]);
}

#[test]
fn ten_times_too_tight_fails() {
    let found = refuted(0.01, TEN, 1.2e-3);
    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("claims 10x"), "{}", found[0]);
    assert_eq!(found[0].worst.unwrap().at, 2.0);
}

#[test]
fn ten_times_too_loose_fails() {
    let found = refuted(0.03, TEN, 1e-3);
    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("over 20x"), "{}", found[0]);
}

#[test]
fn ten_times_observing_nothing_fails() {
    let found = refuted(0.01, TEN, 0.0);
    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("no difference"), "{}", found[0]);
    assert_eq!(found[0].worst, None);
}

#[test]
fn ten_times_below_the_least_margin_fails() {
    let found = refuted(0.01, TEN, 6e-3);
    assert!(
        found[0].message.contains("below the least margin"),
        "{}",
        found[0]
    );
}

const AT_LEAST_3: &str = "observed max 3.0e-3 at t=2 s; bound at least 3x that.";

#[test]
fn at_least_holding_passes() {
    assert_eq!(refuted(0.01, AT_LEAST_3, 3e-3), vec![]);
    assert_eq!(refuted(0.01, AT_LEAST_3, 1e-4), vec![]);
}

#[test]
fn at_least_below_its_margin_fails() {
    let found = refuted(0.01, AT_LEAST_3, 4e-3);
    assert_eq!(found.len(), 1);
    assert!(
        found[0].message.contains("claims at least 3x"),
        "{}",
        found[0]
    );
}

#[test]
fn at_least_below_the_least_margin_fails() {
    let found = refuted(0.01, AT_LEAST_3, 6e-3);
    assert!(
        found[0].message.contains("below the least margin"),
        "{}",
        found[0]
    );
    let found = refuted(0.01, "bound at least 1x that.", 1e-3);
    assert!(
        found[0].message.contains("claims at least 1x"),
        "{}",
        found[0]
    );
}

#[test]
fn rows_without_a_claim_are_not_checked() {
    assert_eq!(refuted(0.01, "Set by the analytic error.", 9e-3), vec![]);
}

#[test]
fn unreadable_or_double_claims_fail() {
    assert!(Claim::parse("bound at least threex that").is_err());
    assert!(Claim::parse("bound at least 3 times").is_err());
    assert!(Claim::parse("bound 10x that; bound at least 3x that").is_err());
    assert_eq!(Claim::parse("bounds 10x those."), Ok(Some(Claim::TenTimes)));
    assert_eq!(
        Claim::parse("bound at least 9x that."),
        Ok(Some(Claim::AtLeast(9)))
    );
}

#[test]
fn relative_rows_measure_relative_margins() {
    let table = ToleranceTable::new(vec![
        row("x", 0.0, 1e-3, TEN),
        row("*", 0.0, 0.0, "Time is exact."),
    ])
    .unwrap();
    let mut margins = Margins::new(&table);
    margins
        .observe_series(
            "c",
            &series(&[100.0, 1.0]),
            &series(&[100.01, 1.0001]),
            &table,
        )
        .unwrap();
    let worst = margins.worst(0).unwrap();
    assert!((worst.margin - 10.0).abs() < 1e-6, "{worst:?}");
    assert_eq!(margins.refuted(&table, &["c".to_owned()]), vec![]);
}

#[test]
fn equal_infinities_are_no_difference_and_one_sided_nan_has_no_margin() {
    let table = ToleranceTable::new(vec![row("*", 1.0, 0.0, TEN)]).unwrap();
    let mut margins = Margins::new(&table);
    let tolerance = &table.rows()[0];
    margins.record(0, tolerance, f64::INFINITY, f64::INFINITY, 0.0);
    margins.record(0, tolerance, f64::NAN, f64::NAN, 0.0);
    assert_eq!(margins.worst(0), None);
    margins.record(0, tolerance, 1.0, f64::NAN, 3.0);
    assert_eq!(margins.worst(0).unwrap().margin, 0.0);
}

#[test]
fn transcripts_use_the_comparators_row_order_and_skip_refusals() {
    let table =
        ToleranceTable::new(vec![row("get/x", 1.0, 0.0, TEN), row("*", 1.0, 0.0, TEN)]).unwrap();
    let mut margins = Margins::new(&table);
    margins
        .observe_transcript(
            "c",
            "get x 1.0\nset y 2.0\nload a => 3\n",
            "get x 1.1\nset y 2.2\nload a => unsupported thing\n",
            &table,
            &["thing".to_owned()],
        )
        .unwrap();
    let (x, rest) = (margins.worst(0).unwrap(), margins.worst(1).unwrap());
    assert_eq!(x.at, 1.0);
    assert!((x.margin - 10.0).abs() < 1e-9, "{x:?}");
    assert_eq!(rest.at, 2.0);
    assert!((rest.margin - 5.0).abs() < 1e-9, "{rest:?}");
}
