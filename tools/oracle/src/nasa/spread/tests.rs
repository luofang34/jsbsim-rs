// SPDX-License-Identifier: LGPL-2.1-or-later

use super::super::data::{Column, Simulator};
use super::{leave_one_out, spread_ratio, two_figures};

const V: Column = Column {
    nasa: "v",
    property: "p",
    scale: 1.0,
    period: None,
};

fn sim(name: &str, rows: &[(f64, f64)]) -> Simulator {
    let mut text = String::from("time,v\n");
    for (t, v) in rows {
        text.push_str(&format!("{t},{v}\n"));
    }
    Simulator::parse(name, &text).unwrap()
}

/// Three simulators: a and b 0.1 apart at most, c 1.0 from a.
fn three() -> Vec<Simulator> {
    vec![
        sim("a", &[(0.0, 0.0), (0.1, 0.0)]),
        sim("b", &[(0.0, 0.05), (0.1, 0.1)]),
        sim("c", &[(0.0, 1.0), (0.1, 1.0)]),
    ]
}

#[test]
fn r_is_the_nearest_simulator_against_the_closest_pair() {
    // Nearest is b: at most 0.2 away (at t=0.1); a is 0.3 away at worst.
    let samples = [(0.0, 0.25), (0.1, 0.3)];
    let r = spread_ratio(&V, &three(), &samples).unwrap();
    assert!((r.nearest - 0.2).abs() < 1e-12, "{r:?}");
    assert!((r.closest_pair - 0.1).abs() < 1e-12, "{r:?}");
    assert!((r.ratio() - 2.0).abs() < 1e-9);
}

#[test]
fn a_candidate_on_a_simulator_has_r_zero_and_one_as_close_as_the_pair_has_r_one() {
    let on_a = [(0.0, 0.0), (0.1, 0.0)];
    assert_eq!(spread_ratio(&V, &three(), &on_a).unwrap().ratio(), 0.0);
    let pair_apart = [(0.0, -0.1), (0.1, -0.1)];
    let r = spread_ratio(&V, &three(), &pair_apart).unwrap().ratio();
    assert!((r - 1.0).abs() < 1e-12, "{r}");
}

#[test]
fn r_takes_the_worst_time_not_the_nearest_simulator_at_each_time() {
    // Near a at t=0 and near c at t=0.1, but no single simulator is near
    // at both: a is 1.0 away at t=0.1 and c 1.0 away at t=0.
    let samples = [(0.0, 0.0), (0.1, 1.0)];
    let r = spread_ratio(&V, &three(), &samples).unwrap();
    assert!((r.nearest - 0.9).abs() < 1e-12, "{r:?}");
}

#[test]
fn r_needs_two_simulators_and_shared_times() {
    let one = vec![sim("a", &[(0.0, 0.0)])];
    assert!(spread_ratio(&V, &one, &[(0.0, 1.0)]).is_none());
    assert!(spread_ratio(&V, &three(), &[(5.0, 1.0)]).is_none());
}

#[test]
fn r_measures_angles_across_the_wrap() {
    let angle = Column {
        period: Some(360.0),
        ..V
    };
    let sims = vec![sim("a", &[(0.0, 359.0)]), sim("b", &[(0.0, 1.0)])];
    let r = spread_ratio(&angle, &sims, &[(0.0, 0.0)]).unwrap();
    assert!((r.closest_pair - 2.0).abs() < 1e-12, "{r:?}");
    assert!((r.ratio() - 0.5).abs() < 1e-12, "{r:?}");
}

#[test]
fn leave_one_out_compares_each_simulator_with_the_others() {
    let sims = vec![
        sim("a", &[(0.0, 0.0)]),
        sim("b", &[(0.0, 2.0)]),
        sim("c", &[(0.0, 1.0)]),
    ];
    let verdicts = leave_one_out(&V, &sims, &[0.0]);
    let expected = [
        ("a".to_owned(), true),
        ("b".to_owned(), true),
        ("c".to_owned(), false),
    ];
    assert_eq!(verdicts, expected);
    // With two simulators, neither has two others to be judged by.
    assert!(leave_one_out(&V, &sims[..2], &[0.0]).is_empty());
}

#[test]
fn two_figures_rounds_to_two_significant_figures() {
    assert_eq!(two_figures(0.0), "0");
    assert_eq!(two_figures(0.034_49), "0.034");
    assert_eq!(two_figures(0.96), "0.96");
    assert_eq!(two_figures(1.04), "1.0");
    assert_eq!(two_figures(21.4), "21");
    assert_eq!(two_figures(232.4), "230");
    assert_eq!(two_figures(1849.0), "1800");
    assert_eq!(two_figures(3.2e-5), "3.2e-5");
    assert_eq!(two_figures(f64::INFINITY), "inf");
}
