// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

#[test]
fn strtod_parses_longest_decimal_prefix() {
    assert_eq!(strtod_prefix("  1.5e3xyz"), (1500.0, 7));
    assert_eq!(strtod_prefix("-.5"), (-0.5, 3));
    assert_eq!(strtod_prefix("1."), (1.0, 2));
    assert_eq!(strtod_prefix("1e"), (1.0, 1));
    assert_eq!(strtod_prefix("1e+"), (1.0, 1));
    assert_eq!(strtod_prefix("abc"), (0.0, 0));
    assert_eq!(strtod_prefix("."), (0.0, 0));
    assert_eq!(strtod_prefix("+"), (0.0, 0));
    assert_eq!(strtod_prefix(""), (0.0, 0));
}

#[test]
fn strtod_handles_special_values() {
    assert_eq!(strtod_prefix("INF"), (f64::INFINITY, 3));
    assert_eq!(strtod_prefix("-Infinity!"), (f64::NEG_INFINITY, 9));
    let (nan, len) = strtod_prefix("nan(123)x");
    assert!(nan.is_nan());
    assert_eq!(len, 8);
    assert_eq!(strtod_prefix("nan(").1, 3);
    assert_eq!(strtod("1e999"), f64::INFINITY);
    assert_eq!(strtod("1e-999"), 0.0);
}

#[test]
fn strtod_parses_hex_floats() {
    assert_eq!(strtod_prefix("0x1p3"), (8.0, 5));
    assert_eq!(strtod_prefix("0x.8"), (0.5, 4));
    assert_eq!(strtod_prefix("0xg"), (0.0, 1));
    assert_eq!(strtod("0x1.fffffffffffff8p0"), 2.0);
    assert_eq!(strtod("0x1.fffffffffffff7p0"), 1.999_999_999_999_999_8);
}

#[test]
fn strtol_follows_c_prefixes_and_saturates() {
    assert_eq!(strtol(" -42abc", 10), -42);
    assert_eq!(strtol("0x1F", 0), 31);
    assert_eq!(strtol("010", 0), 8);
    assert_eq!(strtol("010", 10), 10);
    assert_eq!(strtol("0x", 0), 0);
    assert_eq!(strtol("99999999999999999999", 10), i64::MAX);
    assert_eq!(strtol("-99999999999999999999", 10), i64::MIN);
    assert_eq!(strtol("-9223372036854775808", 10), i64::MIN);
    assert_eq!(atoi("4294967297"), 1);
    assert_eq!(atoi("3.9"), 3);
}

#[test]
fn format_g_matches_printf() {
    assert_eq!(format_g(0.1, 10), "0.1");
    assert_eq!(format_g(1.0 / 3.0, 10), "0.3333333333");
    assert_eq!(format_g(123_456_789_012.0, 10), "1.23456789e+11");
    assert_eq!(format_g(1e-5, 10), "1e-05");
    assert_eq!(format_g(0.0001, 6), "0.0001");
    assert_eq!(format_g(100_000.0, 6), "100000");
    assert_eq!(format_g(1_000_000.0, 6), "1e+06");
    assert_eq!(format_g(999_999.5, 6), "1e+06");
    assert_eq!(format_g(-0.0, 6), "-0");
    assert_eq!(format_g(1e300, 10), "1e+300");
    assert_eq!(format_g(f64::NEG_INFINITY, 6), "-inf");
}

#[test]
fn format_f_matches_printf() {
    assert_eq!(format_f(1.5), "1.500000");
    assert_eq!(format_f(-0.0), "-0.000000");
    assert_eq!(format_f(0.000_000_5), "0.000000");
    assert_eq!(format_f(f64::NAN), "nan");
}
