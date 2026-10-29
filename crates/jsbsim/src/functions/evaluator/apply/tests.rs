// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

#[test]
fn log2_scales_by_the_constant_clang_folds() {
    // `invlog2val` in the oracle build, and what the port computed before
    // folding it: the libm crate's log10(2.0).
    assert_eq!(INV_LOG10_2.to_bits(), 0x400a_934f_0979_a371);
    assert_eq!(INV_LOG10_2.to_bits(), (1.0 / libm::log10(2.0)).to_bits());
    for x in [1e-300, 0.1, 0.5, 2.0, 3.0, 1000.0, 1e300] {
        let runtime = libm::log10(x) * (1.0 / libm::log10(2.0));
        assert_eq!(unary(Op::Log2, x).to_bits(), runtime.to_bits(), "log2({x})");
    }
    assert_eq!(unary(Op::Log2, 0.0), f64::NEG_INFINITY);
}
