// SPDX-License-Identifier: LGPL-2.1-or-later

use super::data::{Column, Simulator};
use super::{
    Archive, Case, Counts, ExpectedMiss, NasaManifest, PARITY_REL, Root, compare_column,
    report_case, report_parity, sha256_hex, verify,
};
use crate::{OracleError, Series};

fn sim(name: &str, rows: &[(f64, f64)]) -> Simulator {
    let mut text = String::from("time,v\n");
    for (t, v) in rows {
        text.push_str(&format!("{t},{v}\n"));
    }
    Simulator::parse(name, &text).unwrap()
}

fn candidate(rows: &[(f64, f64)]) -> Series {
    Series {
        columns: vec!["Time".to_owned(), "p".to_owned()],
        rows: rows.iter().map(|(t, v)| vec![*t, *v]).collect(),
    }
}

const PLAIN: Column = Column {
    nasa: "v",
    property: "p",
    scale: 1.0,
    period: None,
};

#[test]
fn parse_keeps_the_first_of_repeated_columns_and_skips_empty_cells() {
    let s = Simulator::parse("s", "time,a,b,a\n0.0,1,,3\n0.1,4,5,6\n").unwrap();
    assert_eq!(s.value("a", 0), Some(1.0));
    assert_eq!(s.value("a", 100), Some(4.0));
    assert_eq!(s.value("b", 0), None);
    assert_eq!(s.value("b", 100), Some(5.0));
}

#[test]
fn parse_rejects_a_file_without_time_first() {
    assert!(matches!(
        Simulator::parse("s", "t,a\n0,1\n"),
        Err(OracleError::Csv { .. })
    ));
}

#[test]
fn a_candidate_inside_the_spread_passes_and_counts_as_inside() {
    let sims = [sim("a", &[(0.0, 1.0)]), sim("b", &[(0.0, 3.0)])];
    let s = compare_column(&PLAIN, &sims, &candidate(&[(0.0, 2.0)])).unwrap();
    assert_eq!((s.samples, s.inside), (1, 1));
    assert!(s.passes());
}

#[test]
fn a_candidate_outside_the_interval_fails_however_wide_it_is() {
    let sims = [sim("a", &[(0.0, 1.0)]), sim("b", &[(0.0, 3.0)])];
    let s = compare_column(&PLAIN, &sims, &candidate(&[(0.0, 3.0001)])).unwrap();
    assert_eq!((s.inside, s.outside.as_slice()), (0, [0].as_slice()));
    assert!((s.max_excursion - 1e-4).abs() < 1e-12);
    assert!(!s.passes());
}

#[test]
fn printed_precision_is_not_a_difference() {
    let sims = [sim("a", &[(0.0, 1e6)]), sim("b", &[(0.0, 1e6)])];
    let s = compare_column(&PLAIN, &sims, &candidate(&[(0.0, 1e6 + 5e-5)])).unwrap();
    assert!(s.passes());
    let s = compare_column(&PLAIN, &sims, &candidate(&[(0.0, 1e6 + 2e-4)])).unwrap();
    assert!(!s.passes());
}

#[test]
fn each_sample_is_judged_by_its_own_envelope() {
    let sims = [
        sim("a", &[(0.0, 1.0), (0.1, 1.0)]),
        sim("b", &[(0.0, 1.0), (0.1, 3.0)]),
    ];
    let s = compare_column(&PLAIN, &sims, &candidate(&[(0.0, 2.5), (0.1, 2.0)])).unwrap();
    assert_eq!((s.inside, s.outside.as_slice()), (1, [0].as_slice()));
    assert!(!s.passes());
}

#[test]
fn a_candidate_farther_out_than_the_spread_fails() {
    let sims = [sim("a", &[(0.0, 1.0)]), sim("b", &[(0.0, 3.0)])];
    let s = compare_column(&PLAIN, &sims, &candidate(&[(0.0, 5.5)])).unwrap();
    assert!(!s.passes());
    assert_eq!(s.worst_time, 0.0);
}

#[test]
fn angles_compare_across_the_wrap() {
    let column = Column {
        period: Some(360.0),
        ..PLAIN
    };
    let sims = [sim("a", &[(0.0, 179.0)]), sim("b", &[(0.0, -179.0)])];
    let s = compare_column(&column, &sims, &candidate(&[(0.0, 180.0)])).unwrap();
    assert!((s.max_spread - 2.0).abs() < 1e-12);
    assert_eq!(s.inside, 1);
}

#[test]
fn samples_with_fewer_than_two_simulators_are_not_compared() {
    let sims = [sim("a", &[(0.0, 1.0), (0.1, 1.0)]), sim("b", &[(0.0, 1.0)])];
    let s = compare_column(&PLAIN, &sims, &candidate(&[(0.0, 1.0), (0.1, 9.0)])).unwrap();
    assert_eq!(s.samples, 1);
    assert!(s.passes());
}

#[test]
fn verify_rejects_changed_data() {
    let bytes = b"check-case data";
    let archive = Archive {
        name: "x".to_owned(),
        url: String::new(),
        size: bytes.len() as u64,
        sha256: sha256_hex(bytes),
        retrieved: String::new(),
    };
    verify(&archive, bytes).unwrap();
    assert!(matches!(
        verify(&archive, b"check-case dat!"),
        Err(OracleError::NasaData { .. })
    ));
    assert!(matches!(
        verify(&archive, b"short"),
        Err(OracleError::NasaData { .. })
    ));
}

#[test]
fn sha256_matches_a_known_digest() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn the_committed_manifest_is_consistent() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest = NasaManifest::read_blocking(&repo).unwrap();
    for case in &manifest.case {
        manifest.archive(&case.archive).unwrap();
        let root = match case.root {
            Root::Jsbsim => repo.join("oracle").join("inputs"),
            Root::Nasa => repo.join("oracle").join("nasa"),
        };
        assert!(root.join(&case.script).is_file(), "{}", case.script);
        assert!(
            root.join(&case.aircraft_path).is_dir(),
            "{}",
            case.aircraft_path
        );
    }
    for case in &manifest.case {
        if let Some(p) = &case.parity {
            assert!(
                p.rel > PARITY_REL && p.rel < 1e-3,
                "{}: {}",
                case.name,
                p.rel
            );
            assert!(!p.cause.is_empty(), "{}", case.name);
        }
    }
    // The F-16 is a hand translation of the DAVE-ML, so a miss not traced
    // to a cause may be the translation's, not JSBSim's.
    let f16: Vec<_> = manifest
        .case
        .iter()
        .filter(|c| {
            std::fs::read_to_string(repo.join("oracle/nasa").join(&c.script))
                .is_ok_and(|s| s.contains(r#"aircraft="f16""#))
        })
        .collect();
    assert_eq!(f16.len(), 8);
    for case in f16 {
        for miss in &case.expected_miss {
            assert!(
                !miss.cause.starts_with("Not isolated:"),
                "{} {}: an F-16 miss not isolated must say translation error is not excluded",
                case.name,
                miss.variable
            );
        }
    }
    for archive in &manifest.archive {
        assert_eq!(archive.sha256.len(), 64, "{}", archive.name);
        assert!(archive.url.starts_with("https://"), "{}", archive.name);
    }
}

#[test]
fn parity_holds_each_property_to_the_case_bound_of_its_magnitude() {
    let cpp = Series {
        columns: vec!["Time".to_owned(), "p".to_owned()],
        rows: vec![vec![0.0, 1000.0], vec![0.1, 2000.0]],
    };
    let mut rust = cpp.clone();
    rust.rows[0][1] += 2e-6;
    assert_eq!(report_parity("c", PARITY_REL, &cpp, &rust), 1);
    assert_eq!(report_parity("c", 1e-9, &cpp, &rust), 0);
    assert_eq!(report_parity("c", 1e-10, &cpp, &rust), 1);
}

#[test]
fn a_case_states_a_looser_parity_with_its_cause() {
    let text = r#"
archive = []
[[case]]
name = "c"
archive = "a"
root = "nasa"
script = "s.xml"
aircraft_path = "aircraft"
section = "s"
[case.parity]
rel = 1e-9
cause = "why"
[[case]]
name = "d"
archive = "a"
root = "nasa"
script = "s.xml"
aircraft_path = "aircraft"
section = "s"
"#;
    let manifest: NasaManifest = toml::from_str(text).unwrap();
    assert_eq!(manifest.case[0].parity_rel(), 1e-9);
    assert_eq!(manifest.case[1].parity_rel(), PARITY_REL);
}

/// Two simulators recording `a` to `d` as 1 and 3 at t=0.
fn four_variable_sims() -> Vec<Simulator> {
    [("s1", 1), ("s2", 3)]
        .iter()
        .map(|(name, v)| {
            Simulator::parse(name, &format!("time,a,b,c,d\n0.0,{v},{v},{v},{v}\n")).unwrap()
        })
        .collect()
}

fn four_variable_series(values: [f64; 4]) -> Series {
    Series {
        columns: ["Time", "pa", "pb", "pc", "pd"].map(str::to_owned).to_vec(),
        rows: vec![std::iter::once(0.0).chain(values).collect()],
    }
}

const FOUR_COLUMNS: [Column; 5] = [
    Column {
        nasa: "a",
        property: "pa",
        ..PLAIN
    },
    Column {
        nasa: "b",
        property: "pb",
        ..PLAIN
    },
    Column {
        nasa: "c",
        property: "pc",
        ..PLAIN
    },
    Column {
        nasa: "d",
        property: "pd",
        ..PLAIN
    },
    // No simulator records it, so it is not compared.
    Column {
        nasa: "e",
        property: "pa",
        ..PLAIN
    },
];

fn case_missing(variables: &[&str]) -> Case {
    Case {
        name: "c".to_owned(),
        archive: "a".to_owned(),
        root: Root::Nasa,
        script: "s.xml".to_owned(),
        aircraft_path: "aircraft".to_owned(),
        section: "s".to_owned(),
        expected_miss: variables
            .iter()
            .map(|v| ExpectedMiss {
                variable: (*v).to_owned(),
                cause: "why".to_owned(),
            })
            .collect(),
        parity: None,
    }
}

#[test]
fn each_compared_variable_is_counted_once_by_its_verdict() {
    // a inside, b outside and listed, c inside but listed, d outside.
    let series = four_variable_series([2.0, 9.0, 2.0, 9.0]);
    let sims = four_variable_sims();
    let counts = report_case(
        &case_missing(&["b", "c"]),
        &FOUR_COLUMNS,
        &sims,
        &series,
        &series,
    );
    let expected = Counts {
        pass: 1,
        expected_miss: 1,
        unexpected_pass: 1,
        fail: 1,
        ..Counts::default()
    };
    assert_eq!(counts, expected);
    assert!(!counts.ok());
    let clean = report_case(
        &case_missing(&["b", "d"]),
        &FOUR_COLUMNS,
        &sims,
        &series,
        &series,
    );
    assert!(clean.ok(), "{clean}");
    assert_eq!((clean.pass, clean.expected_miss), (2, 2));
}

#[test]
fn an_expected_miss_of_a_variable_not_compared_fails_as_stale() {
    let series = four_variable_series([2.0, 9.0, 2.0, 9.0]);
    // "e" is a column no simulator records; "dd" is misspelt.
    let case = case_missing(&["b", "d", "e", "dd"]);
    let counts = report_case(
        &case,
        &FOUR_COLUMNS,
        &four_variable_sims(),
        &series,
        &series,
    );
    assert_eq!((counts.expected_miss, counts.stale), (2, 2));
    assert!(!counts.ok());
}

#[test]
fn rust_missing_on_other_samples_than_cpp_is_a_fail() {
    let cpp = four_variable_series([2.0, 9.0, 2.0, 9.0]);
    let rust = four_variable_series([9.0, 9.0, 2.0, 9.0]);
    let case = case_missing(&["b", "d"]);
    let counts = report_case(&case, &FOUR_COLUMNS, &four_variable_sims(), &cpp, &rust);
    assert_eq!((counts.pass, counts.fail, counts.expected_miss), (1, 1, 2));
    assert!(!counts.ok());
}

#[test]
fn counts_total_across_cases() {
    let mut total = Counts::default();
    total += Counts {
        pass: 2,
        expected_miss: 1,
        ..Counts::default()
    };
    total += Counts {
        pass: 1,
        parity_fail: 3,
        rust_stopped: 1,
        ..Counts::default()
    };
    let expected = Counts {
        pass: 3,
        expected_miss: 1,
        parity_fail: 3,
        rust_stopped: 1,
        ..Counts::default()
    };
    assert_eq!(total, expected);
    assert!(!total.ok());
    assert_eq!(
        total.to_string(),
        "3 pass, 1 expected_miss, 0 unexpected_pass, 0 fail, 0 stale expected_miss, \
         3 parity fail, 1 Rust run stopped"
    );
}
