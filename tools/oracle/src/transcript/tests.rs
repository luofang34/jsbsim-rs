// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

fn table(rows: &[(&str, f64)]) -> ToleranceTable {
    ToleranceTable::new(
        rows.iter()
            .map(|(property, rel)| Tolerance {
                case: "c".into(),
                property: (*property).into(),
                abs: 0.0,
                rel: *rel,
                period: None,
                t_start: None,
                t_end: None,
                reason: "test".into(),
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn identical_transcripts_agree_without_any_table() {
    let outcome = compare_transcript("c", "a 1\nb 2\n", "a 1\nb 2\n", &table(&[])).unwrap();
    assert_eq!(outcome.divergence, None);
    assert_eq!(outcome.lines, 2);
    assert_eq!(outcome.numbers_differing, 0);
}

#[test]
fn without_rows_any_difference_diverges_at_its_line() {
    let outcome = compare_transcript("c", "a 1\nb 2\n", "a 1\nb 2.0000001\n", &table(&[])).unwrap();
    assert_eq!(outcome.divergence.unwrap().line, 2);
}

#[test]
fn missing_and_extra_lines_diverge() {
    let t = table(&[("*", 1.0)]);
    assert_eq!(
        compare_transcript("c", "a 1\nb 2\n", "a 1\n", &t)
            .unwrap()
            .divergence
            .unwrap()
            .line,
        2
    );
    assert!(
        compare_transcript("c", "a 1\n", "a 1\nb\n", &t)
            .unwrap()
            .divergence
            .is_some()
    );
}

#[test]
fn numbers_use_op_tag_rows_before_the_wildcard() {
    let t = table(&[("quat/euler", 1e-12), ("*", 0.0)]);
    let r = "quat 1 => euler 1.0 mag 2.0\n";
    let within = "quat 1 => euler 1.0000000000001 mag 2.0\n";
    let outcome = compare_transcript("c", r, within, &t).unwrap();
    assert_eq!(outcome.divergence, None);
    assert_eq!(outcome.numbers_differing, 1);
    assert!(outcome.max_rel > 0.0 && outcome.max_rel < 1e-12);
    let mag_off = "quat 1 => euler 1.0 mag 2.0000000000001\n";
    let divergence = compare_transcript("c", r, mag_off, &t)
        .unwrap()
        .divergence
        .unwrap();
    assert!(divergence.detail.starts_with("quat/mag:"), "{divergence}");
}

#[test]
fn text_tokens_must_match_exactly() {
    let t = table(&[("*", 1.0)]);
    let outcome = compare_transcript("c", "a => x 1\n", "a => y 1\n", &t).unwrap();
    assert!(outcome.divergence.unwrap().detail.contains("token"));
}

#[test]
fn nan_on_one_side_diverges() {
    let t = table(&[("*", 1.0)]);
    let outcome = compare_transcript("c", "a => v 1\n", "a => v nan\n", &t).unwrap();
    assert!(outcome.divergence.is_some());
}

#[test]
fn rows_of_other_cases_do_not_apply() {
    let t = table(&[("*", 1.0)]);
    let err = compare_transcript("c", "a => v 1\n", "a => v 2\n", &t);
    assert!(err.unwrap().divergence.is_none());
    let other = compare_transcript("d", "a => v 1\n", "a => v 2\n", &t).unwrap();
    assert!(other.divergence.is_some(), "case d has no rows: exact");
}

#[test]
fn op_wide_rows_sit_between_tag_rows_and_the_wildcard() {
    let t = table(&[("quat/*", 0.0), ("*", 1.0)]);
    let outcome = compare_transcript("c", "quat => mag 1\n", "quat => mag 1.5\n", &t).unwrap();
    assert!(outcome.divergence.is_some(), "quat/* is exact");
    let outcome = compare_transcript("c", "vec => mag 1\n", "vec => mag 1.5\n", &t).unwrap();
    assert!(outcome.divergence.is_none(), "vec falls back to *");
}
