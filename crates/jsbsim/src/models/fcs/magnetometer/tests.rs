// SPDX-License-Identifier: LGPL-2.1-or-later
use super::gmtime_date;
use super::magvar::yymmdd_to_julian_days;

#[test]
fn gmtime_dates_count_years_from_1900_and_months_from_0() {
    assert_eq!(gmtime_date(0), (70, 0, 1));
    assert_eq!(gmtime_date(-1), (69, 11, 31));
    assert_eq!(gmtime_date(951_782_400), (100, 1, 29));
    assert_eq!(gmtime_date(1_276_603_200), (110, 5, 15));
    assert_eq!(gmtime_date(1_767_225_600 - 1), (125, 11, 31));
    assert_eq!(gmtime_date(1_767_225_600), (126, 0, 1));
}

#[test]
fn julian_days_follow_the_c_integer_arithmetic() {
    // 1 January 2005, the WMM2005 epoch, and 1 January 2000.
    assert_eq!(yymmdd_to_julian_days(5, 1, 1), 2_453_372);
    assert_eq!(yymmdd_to_julian_days(0, 1, 1), 2_451_545);
    // A year since 1900 above 49, as gmtime gives it, and a month 0.
    assert_eq!(yymmdd_to_julian_days(126, 0, 1), 2_461_011);
}
