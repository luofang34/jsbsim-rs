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

/// Four uniform then five normal deviates (the fifth served from the
/// cache), as bits, for seeds 0, 12345 and 2^31 - 2. They are pinned
/// because every target must draw the same bits; a target whose float
/// arithmetic or `log` differs fails here.
#[test]
fn draws_are_the_same_bits_on_every_target() {
    let pins: [(u32, [u64; 4], [u64; 5]); 3] = [
        (
            0,
            [
                0xbfea8ed40f6a3a94,
                0x3fe91028bf3139c4,
                0xbfe3dc1f6e538a85,
                0xbfca1c1f4d60c598,
            ],
            [
                0x3ff9caca676b8475,
                0x3fd995d7af7c24c6,
                0x3ff61ff4a4d244de,
                0x3faac985a35d025a,
                0xbfeceec4a6fbd059,
            ],
        ),
        (
            12_345,
            [
                0x3fdcdff56bba8970,
                0x3fec3cdfea4a48b4,
                0x3fdc3099f0da8600,
                0xbfc62171fb97bab4,
            ],
            [
                0xbfe1c759202f6e5b,
                0x3fef1ecf54dc9b39,
                0x3fe4b51be0cc080e,
                0xbff8825027331d18,
                0x3fe5dbd5cc9a3abe,
            ],
        ),
        (
            2_147_483_646,
            [
                0x3fea8ed40f6a3a94,
                0xbfe91028bf3139c2,
                0x3fe3dc1f6e538a86,
                0x3fca1c1f4d60c590,
            ],
            [
                0xbff9caca676b8476,
                0xbfd995d7af7c24ba,
                0xbff61ff4a4d244de,
                0xbfaac985a35d0280,
                0x3feceec4a6fbd05e,
            ],
        ),
    ];
    for (seed, uniform, normal) in pins {
        let g = RandomNumberGenerator::new(seed);
        let drawn: Vec<u64> = (0..4).map(|_| g.uniform().to_bits()).collect();
        assert_eq!(drawn, uniform, "uniform, seed {seed}");
        let drawn: Vec<u64> = (0..5).map(|_| g.normal().to_bits()).collect();
        assert_eq!(drawn, normal, "normal, seed {seed}");
    }
}

/// Reseeding mid-sequence, with a normal deviate cached: seed 42 draws one
/// normal, seed 9 three normals and a uniform, then seed 42 again two
/// normals, the first of which repeats the very first draw.
#[test]
fn reseeded_draws_are_the_same_bits_on_every_target() {
    let g = RandomNumberGenerator::new(42);
    let mut drawn = vec![g.normal().to_bits()];
    g.seed(9);
    drawn.extend((0..3).map(|_| g.normal().to_bits()));
    drawn.push(g.uniform().to_bits());
    g.seed(42);
    drawn.extend((0..2).map(|_| g.normal().to_bits()));
    assert_eq!(
        drawn,
        [
            0x4000faca0b529d0d,
            0x3ffabe3de1dee247,
            0x3fe52f9ef05e4546,
            0x3fb3b76e33714779,
            0xbfdb611444300f44,
            0x4000faca0b529d0d,
            0xbff8e682c07ed003,
        ]
    );
}
