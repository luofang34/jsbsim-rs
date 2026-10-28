// SPDX-License-Identifier: LGPL-2.1-or-later

use super::data::{Column, Simulator};
use super::{Archive, NasaManifest, Root, compare_column, sha256_hex, verify};
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
    for archive in &manifest.archive {
        assert_eq!(archive.sha256.len(), 64, "{}", archive.name);
        assert!(archive.url.starts_with("https://"), "{}", archive.name);
    }
}
