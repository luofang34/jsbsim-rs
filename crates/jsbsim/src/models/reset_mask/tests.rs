// SPDX-License-Identifier: LGPL-2.1-or-later

use super::ResetMask;

#[test]
fn constants_have_jsbsims_values() {
    assert_eq!(ResetMask::START_NEW_OUTPUT.bits(), 0x1);
    assert_eq!(ResetMask::DONT_EXECUTE_RUN_IC.bits(), 0x2);
    assert_eq!(ResetMask::empty(), ResetMask::default());
    assert_eq!(ResetMask::empty().bits(), 0);
}

#[test]
fn or_combines_flags_and_contains_tests_each() {
    let both = ResetMask::START_NEW_OUTPUT | ResetMask::DONT_EXECUTE_RUN_IC;
    assert_eq!(both.bits(), 0x3);
    assert!(both.contains(ResetMask::START_NEW_OUTPUT));
    assert!(both.contains(ResetMask::DONT_EXECUTE_RUN_IC));
    assert!(both.contains(both));
    assert!(!ResetMask::START_NEW_OUTPUT.contains(ResetMask::DONT_EXECUTE_RUN_IC));
    assert!(!ResetMask::START_NEW_OUTPUT.contains(both));
    assert!(ResetMask::empty().contains(ResetMask::empty()));
    assert!(!ResetMask::empty().contains(ResetMask::DONT_EXECUTE_RUN_IC));

    let mut m = ResetMask::empty();
    m |= ResetMask::DONT_EXECUTE_RUN_IC;
    assert_eq!(m, ResetMask::DONT_EXECUTE_RUN_IC);
    m |= ResetMask::START_NEW_OUTPUT;
    assert_eq!(m, both);
}

#[test]
fn unknown_bits_are_retained() {
    let m = ResetMask::from_bits_retain(0xFF);
    assert_eq!(m.bits(), 0xFF);
    assert!(m.contains(ResetMask::DONT_EXECUTE_RUN_IC));
    assert!(ResetMask::from_bits_retain(2).contains(ResetMask::DONT_EXECUTE_RUN_IC));
    assert!(!ResetMask::from_bits_retain(4).contains(ResetMask::DONT_EXECUTE_RUN_IC));
    // A negative `int` from `simulation/reset` keeps its two's complement bits.
    assert!(ResetMask::from_bits_retain(-1_i32 as u32).contains(ResetMask::DONT_EXECUTE_RUN_IC));
}
