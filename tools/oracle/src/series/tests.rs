// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

#[test]
fn parses_header_and_rows() {
    let s = Series::parse("Time,a/b\n0,1.5\n0.5,-2e-3\n", "t").unwrap();
    assert_eq!(s.columns, ["Time", "a/b"]);
    assert_eq!(s.rows, [vec![0.0, 1.5], vec![0.5, -2e-3]]);
    assert_eq!(s.time(1), Some(0.5));
    assert_eq!(s.time(2), None);
}

#[test]
fn round_trips_full_precision() {
    let s = Series {
        columns: vec!["Time".into(), "x".into()],
        rows: vec![vec![0.1, 1.0 / 3.0], vec![1e300, -5e-324]],
    };
    assert_eq!(Series::parse(&s.to_csv(), "t").unwrap(), s);
}

#[test]
fn accepts_c_printf_g_output() {
    let s = Series::parse("Time,x\n1.0000000000000007,1.3977903401843428e-17\n", "t").unwrap();
    assert_eq!(s.rows[0][0], 1.000_000_000_000_000_7);
    assert_eq!(s.rows[0][1], 1.397_790_340_184_342_8e-17);
}

#[test]
fn rejects_ragged_row_with_line_number() {
    let err = Series::parse("Time,x\n0,1\n1\n", "f.csv").unwrap_err();
    assert_eq!(err.to_string(), "f.csv:3: 1 cells, header has 2");
}

#[test]
fn rejects_non_numeric_cell() {
    let err = Series::parse("Time,x\n0,abc\n", "f.csv").unwrap_err();
    assert!(err.to_string().starts_with("f.csv:2: \"abc\""), "{err}");
}

#[test]
fn requires_time_first() {
    assert!(Series::parse("x,Time\n", "f").is_err());
    assert!(Series::parse("", "f").is_err());
}
