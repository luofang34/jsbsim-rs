// SPDX-License-Identifier: LGPL-2.1-or-later

use super::super::data::{Column, Simulator};
use super::super::report::report_case;
use super::super::{Case, ExpectedMiss, Parity, Root};
use super::{accepted_flips, threshold_distance};
use crate::Series;

const V: Column = Column {
    nasa: "v",
    property: "p",
    scale: 1.0,
    period: None,
};

/// Two simulators at 1 and 3 at t=0: the interval [1, 3], and a printed
/// precision floor of 3e-10.
fn sims() -> Vec<Simulator> {
    ["s1,1", "s2,3"]
        .iter()
        .map(|s| {
            let (name, v) = s.split_once(',').unwrap();
            Simulator::parse(name, &format!("time,v\n0.0,{v}\n")).unwrap()
        })
        .collect()
}

fn series(v: f64) -> Series {
    Series {
        columns: vec!["Time".to_owned(), "p".to_owned()],
        rows: vec![vec![0.0, v]],
    }
}

/// C++ inside by 1e-10 of the threshold, Rust outside by 1e-10: they
/// differ by 2e-10. With `rel`, the bound is `rel` x 3 (the magnitude).
fn flip(rel: f64, cpp: f64, rust: f64) -> Option<usize> {
    let (c, r) = (series(cpp), series(rust));
    let c_out: Vec<i64> = if cpp > 3.0 + 3e-10 { vec![0] } else { vec![] };
    let r_out: Vec<i64> = if rust > 3.0 + 3e-10 { vec![0] } else { vec![] };
    accepted_flips(&V, &sims(), (&c, &r), (&c_out, &r_out), rel).map(|f| f.len())
}

#[test]
fn the_threshold_is_the_interval_widened_by_printed_precision() {
    let d = threshold_distance(&V, &sims(), 0, 3.0 + 2e-10).unwrap();
    assert!((d - 1e-10).abs() < 1e-15, "{d}");
    let d = threshold_distance(&V, &sims(), 0, 2.0).unwrap();
    assert!((d - 3e-10).abs() < 1e-15, "{d}");
    assert!(threshold_distance(&V, &sims(), 100, 2.0).is_none());
}

#[test]
fn a_flip_just_inside_the_bound_is_accepted_and_counted() {
    // Bound 3e-9: C++ 1e-10 from the threshold, Rust 2e-10 from C++.
    assert_eq!(flip(1e-9, 3.0 + 2e-10, 3.0 + 4e-10), Some(1));
    // Agreeing samples are no flips.
    assert_eq!(flip(1e-9, 2.0, 2.0), Some(0));
}

#[test]
fn a_flip_just_outside_the_bound_fails() {
    // Bound 9e-11, below C++'s 1e-10 from the threshold.
    assert_eq!(flip(3e-11, 3.0 + 2e-10, 3.0 + 4e-10), None);
}

#[test]
fn rust_farther_from_cpp_than_the_bound_fails_even_with_cpp_at_the_threshold() {
    // C++ 1e-13 from the threshold, Rust 1e-6 away from C++; bound 3e-9.
    assert_eq!(flip(1e-9, 3.0 + 3e-10 - 1e-13, 3.0 + 1e-6), None);
}

fn case(cap: usize) -> Case {
    Case {
        name: "c".to_owned(),
        archive: "a".to_owned(),
        root: Root::Nasa,
        script: "s.xml".to_owned(),
        aircraft_path: "aircraft".to_owned(),
        section: "s".to_owned(),
        expected_miss: Vec::<ExpectedMiss>::new(),
        parity: Some(Parity {
            rel: 1e-9,
            cause: "why".to_owned(),
            max_threshold_flips: cap,
            threshold_flips_cause: "this sample".to_owned(),
        }),
    }
}

#[test]
fn accepted_flips_are_counted_per_case_and_fail_beyond_its_cap() {
    // C++ inside, Rust outside: the variable passes on C++'s verdict.
    let (cpp, rust) = (series(3.0 + 2e-10), series(3.0 + 4e-10));
    let allowed = report_case(&case(1), &[V], &sims(), &cpp, &rust);
    assert_eq!((allowed.pass, allowed.threshold_flips), (1, 1));
    assert!(allowed.ok(), "{allowed}");
    let over = report_case(&case(0), &[V], &sims(), &cpp, &rust);
    assert_eq!((over.threshold_flips, over.flips_over_cap), (1, 1));
    assert!(!over.ok());
}

#[test]
fn a_cap_above_the_flips_accepted_is_stale() {
    let agreeing = series(2.0);
    let counts = report_case(&case(1), &[V], &sims(), &agreeing, &agreeing);
    assert_eq!((counts.threshold_flips, counts.stale), (0, 1));
    assert!(!counts.ok());
    assert!(report_case(&case(0), &[V], &sims(), &agreeing, &agreeing).ok());
}
