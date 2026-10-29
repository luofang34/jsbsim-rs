// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

/// What a program built with Apple clang and libc++ prints for
/// `std::default_random_engine g(0u)`, three `uniform_real_distribution
/// (-1, 1)` deviates, three `normal_distribution(0, 1)` deviates, then
/// `g()`, each through `%.17g`.
#[test]
fn seed_zero_matches_libcxx() {
    let g = RandomNumberGenerator::new(0);
    let printed = |v: f64| crate::c_numeric::format_g(v, 17);
    let uniform: Vec<String> = (0..3).map(|_| printed(g.uniform())).collect();
    assert_eq!(
        uniform,
        [
            "-0.8299351025651327",
            "0.78322255460971535",
            "-0.62062045620071216"
        ]
    );
    let normal: Vec<String> = (0..3).map(|_| printed(g.normal())).collect();
    assert_eq!(
        normal,
        [
            "-0.61744669856356205",
            "1.4741992652628657",
            "0.25993653856957988"
        ]
    );
    assert_eq!(g.next_u32(), 1_271_135_913);
}

#[test]
fn schrage_is_the_exact_modular_product() {
    for x in [1, 2, 44_488, 44_489, 1_000_000, MODULUS - 2, MODULUS - 1] {
        let exact = (u64::from(MULTIPLIER) * u64::from(x)) % u64::from(MODULUS);
        assert_eq!(u64::from(engine_next(x)), exact, "x = {x}");
    }
}

#[test]
fn multiples_of_the_modulus_seed_one() {
    assert_eq!(engine_state(0), 1);
    assert_eq!(engine_state(MODULUS), 1);
    assert_eq!(engine_state(MODULUS + 5), 5);
    // `SRand(int)` stores a negative seed into an `unsigned int`.
    assert_eq!(engine_state(-1_i32 as u32), u32::MAX % MODULUS);
}

#[test]
fn reseeding_drops_the_cached_normal() {
    let g = RandomNumberGenerator::new(7);
    let first = g.normal();
    g.seed(7);
    assert_eq!(g.normal().to_bits(), first.to_bits());
    let fresh = RandomNumberGenerator::new(7);
    assert_eq!(fresh.normal().to_bits(), first.to_bits());
}

#[test]
fn a_clone_continues_the_same_sequence() {
    let g = RandomNumberGenerator::new(3);
    let _first = g.normal();
    let copy = g.clone();
    assert_eq!(g.normal().to_bits(), copy.normal().to_bits());
    assert_eq!(g.uniform().to_bits(), copy.uniform().to_bits());
}
