// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

const SPEC_ONLY: &str = r#"
[case]
script = "scripts/ball.xml"
rate_hz = 0.1
properties = ["position/h-sl-ft"]
"#;

#[test]
fn parses_spec_without_provenance() {
    let m = Manifest::parse(SPEC_ONLY, Path::new("m.toml")).unwrap();
    assert_eq!(m.case.script, "scripts/ball.xml");
    assert_eq!(m.case.end_time, None);
    assert_eq!(m.provenance, None);
    assert_eq!(m.expected_columns(), ["Time", "position/h-sl-ft"]);
}

#[test]
fn round_trips_through_files() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = Manifest::parse(SPEC_ONLY, Path::new("m.toml")).unwrap();
    m.case.end_time = Some(5.0);
    m.provenance = Some(Provenance {
        upstream_commit: "abc".into(),
        driver: "tools/oracle/cpp/oracle_driver.cpp".into(),
        dt: 0.01,
        every_n_frames: 1000,
        rows: 2,
    });
    m.write_blocking(dir.path()).unwrap();
    assert_eq!(Manifest::read_blocking(dir.path()).unwrap(), m);
}

#[test]
fn unknown_keys_are_rejected() {
    let text = format!("{SPEC_ONLY}tolerance = 1\n");
    assert!(Manifest::parse(&text, Path::new("m.toml")).is_err());
}

#[test]
fn lists_only_directories_with_manifests() {
    let dir = tempfile::tempdir().unwrap();
    for case in ["b", "a"] {
        std::fs::create_dir(dir.path().join(case)).unwrap();
        std::fs::write(dir.path().join(case).join(MANIFEST_FILE), SPEC_ONLY).unwrap();
    }
    std::fs::create_dir(dir.path().join("empty")).unwrap();
    assert_eq!(list_cases_blocking(dir.path()).unwrap(), ["a", "b"]);
}
