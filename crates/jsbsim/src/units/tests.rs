// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

#[test]
fn derived_factors_follow_their_definitions() {
    let t = UnitTable::new();
    assert_eq!(t.factor("M", "FT"), Some(3.2808399));
    assert_eq!(t.factor("FT", "M"), Some(1.0 / 3.2808399));
    assert_eq!(t.factor("IN", "M"), Some((1.0 / 12.0) * (1.0 / 3.2808399)));
    assert_eq!(t.factor("RAD", "DEG"), Some(RADTODEG));
    assert_eq!(t.factor("DEG", "RAD"), Some(1.0 / RADTODEG));
    assert_eq!(t.factor("M/S", "KTS"), Some(3.2808399 / 1.6878098571));
}

#[test]
fn identities_and_gaps() {
    let t = UnitTable::new();
    assert_eq!(t.factor("VOLTS", "VOLTS"), Some(1.0));
    assert_eq!(t.factor("KG/SEC", "KG/SEC"), None);
    assert_eq!(t.factor("FT", "LBS"), None);
    assert!(t.knows("KG/SEC"));
    assert!(!t.knows("FURLONG"));
}

#[test]
fn helpers_match_jsbsim_definitions() {
    assert_eq!(constrain(0.0, -1.0, 1.0), 0.0);
    assert_eq!(constrain(0.0, 2.0, 1.0), 1.0);
    assert_eq!(sign(-0.0), 1.0);
    assert_eq!(sign(-1e-300), -1.0);
    assert!(equal_to_roundoff(0.1 + 0.2, 0.3));
    assert!(!equal_to_roundoff(1.0, 1.0 + 1e-10));
    assert_eq!(celsius_to_kelvin(0.0), 273.15);
}
