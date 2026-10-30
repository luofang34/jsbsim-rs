// SPDX-License-Identifier: LGPL-2.1-or-later

use super::{AFTER_S, MAX_DEG_S, ROLL_RATE, judge, max_after};
use crate::Series;

fn roll(rows: &[(f64, f64)]) -> Series {
    Series {
        columns: vec!["Time".to_owned(), ROLL_RATE.to_owned()],
        rows: rows.iter().map(|&(t, p)| vec![t, p]).collect(),
    }
}

const DEG: f64 = std::f64::consts::PI / 180.0;

#[test]
fn only_samples_after_the_transient_count() {
    let s = roll(&[(5.0, 30.0 * DEG), (10.5, -0.004 * DEG), (11.0, 0.002 * DEG)]);
    let (p, t) = max_after(&s, ROLL_RATE, AFTER_S, 1.0 / DEG).unwrap();
    assert!((p - 0.004).abs() < 1e-12 && t == 10.5, "{p} {t}");
    assert!(judge("test", &s));
    assert!(max_after(&s, ROLL_RATE, 20.0, 1.0).is_none());
    assert!(max_after(&s, "other", 0.0, 1.0).is_none());
}

#[test]
fn a_limit_cycle_fails_and_so_does_a_run_ending_before_it_is_judged() {
    let cycle = roll(&[(10.1, 0.0025 * DEG), (12.0, -31.0 * DEG)]);
    assert!(!judge("test", &cycle));
    let at_bound = roll(&[(10.1, MAX_DEG_S * DEG)]);
    assert!(!judge("test", &at_bound));
    assert!(!judge("test", &roll(&[(1.0, 0.0)])));
    assert!(!judge("test", &roll(&[(10.1, f64::NAN)])));
}
