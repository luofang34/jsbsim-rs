// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

#[test]
fn identical_transcripts_have_no_difference() {
    assert_eq!(first_line_difference("a\nb\n", "a\nb\n"), None);
}

#[test]
fn reports_first_differing_line() {
    assert_eq!(
        first_line_difference("a\nb\nc\n", "a\nx\nc\n"),
        Some((2, "b".into(), "x".into()))
    );
}

#[test]
fn reports_missing_lines() {
    assert_eq!(
        first_line_difference("a\nb\n", "a\n"),
        Some((2, "b".into(), String::new()))
    );
    assert_eq!(
        first_line_difference("a\n", "a\nb\n"),
        Some((2, String::new(), "b".into()))
    );
}
