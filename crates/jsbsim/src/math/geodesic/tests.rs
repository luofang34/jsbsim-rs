// SPDX-License-Identifier: LGPL-2.1-or-later
//! Checks of the inverse problem against GeographicLib's published WGS-84
//! results; the oracle's geodesic golden compares it with the C++ library
//! bit for bit.

use super::Geodesic;

fn wgs84() -> Geodesic {
    Geodesic::new(6_378_137.0, 1.0 / 298.257_223_563)
}

#[test]
fn jfk_to_lhr_matches_geodsolve() {
    // `echo 40.6 -73.8 51.6 -0.5 | GeodSolve -i`.
    let g = wgs84();
    let s12 = g.inverse_distance(40.6, -73.8, 51.6, -0.5);
    let (azi1, azi2) = g.inverse_azimuths(40.6, -73.8, 51.6, -0.5);
    assert!((s12 - 5_551_759.400_319).abs() < 1e-6, "{s12}");
    assert!((azi1 - 51.198_882_845).abs() < 1e-9, "{azi1}");
    assert!((azi2 - 107.821_776_735).abs() < 1e-9, "{azi2}");
}

#[test]
fn quarter_meridian_and_equator() {
    let g = wgs84();
    let meridian = g.inverse_distance(0.0, 0.0, 90.0, 0.0);
    assert!((meridian - 10_001_965.729_313).abs() < 1e-6, "{meridian}");
    let (azi1, _) = g.inverse_azimuths(0.0, 0.0, 90.0, 0.0);
    assert_eq!(azi1, 0.0);
    let equator = g.inverse_distance(0.0, 0.0, 0.0, 90.0);
    assert_eq!(equator, 6_378_137.0 * core::f64::consts::FRAC_PI_2);
    let (azi1, azi2) = g.inverse_azimuths(0.0, 0.0, 0.0, 90.0);
    assert_eq!((azi1, azi2), (90.0, 90.0));
}

#[test]
fn equatorial_antipodes_go_over_a_pole() {
    let g = wgs84();
    let s12 = g.inverse_distance(0.0, 0.0, 0.0, 180.0);
    assert!((s12 - 2.0 * 10_001_965.729_313).abs() < 1e-5, "{s12}");
}

#[test]
fn coincident_points_are_zero_apart() {
    let g = wgs84();
    assert_eq!(g.inverse_distance(30.0, 20.0, 30.0, 20.0), 0.0);
    assert_eq!(g.inverse_distance(-90.0, 0.0, -90.0, 120.0), 0.0);
}

#[test]
fn reversing_a_pair_keeps_the_distance_on_oblate_and_prolate_ellipsoids() {
    for g in [wgs84(), Geodesic::new(2.0e7, -0.015)] {
        for (lat1, lon1, lat2, lon2) in [
            (10.0, 20.0, -35.0, 150.0),
            (-60.0, -170.0, 59.5, 9.0),
            (0.5, 0.0, -0.4, 179.7),
        ] {
            let there = g.inverse_distance(lat1, lon1, lat2, lon2);
            let back = g.inverse_distance(lat2, lon2, lat1, lon1);
            assert!((there - back).abs() <= 1e-8 * there, "{there} {back}");
        }
    }
}

/// The bits `Location::distance_to` and `heading_to` give on JSBSim's
/// WGS-84 ellipse for random, near-antipodal and tiny-separation pairs.
/// They are pinned because every target must compute the same bits (the
/// libm crate, not platform maths); a target that differs fails here.
#[test]
fn distances_and_headings_are_the_same_bits_on_every_target() {
    let pins: [(u64, u64); 6] = [
        (0x4181a648758f201f, 0xc00608dd4fc3338d),
        (0x4184c61e0c269b74, 0xbff99ca8290004b4),
        (0x418f4b700302a6c1, 0xc00921fb51d2b861),
        (0x418f4b51f7bfcfc4, 0xc009217dfcecfa38),
        (0x3ff26ee8c9820c85, 0xbfd215b4aa219b2d),
        (0x3fa82c0e3c8faa8b, 0x3fdccba55567e073),
    ];
    for ((lon1, lat1, h, lon2, lat2), (s, psi)) in PINNED_PAIRS.into_iter().zip(pins) {
        let mut l = crate::math::Location::default();
        l.set_ellipse(20_925_646.325_46, 20_855_486.595_293_265);
        l.set_position_geodetic(lon1, lat1, h);
        assert_eq!(l.distance_to(lon2, lat2).to_bits(), s, "{lon1} {lat1}");
        assert_eq!(l.heading_to(lon2, lat2).to_bits(), psi, "{lon1} {lat1}");
    }
}

/// Pairs from the geodesic golden's input.
#[cfg(test)]
const PINNED_PAIRS: [(f64, f64, f64, f64, f64); 6] = [
    (
        2.9689007974638644,
        0.11723052177444813,
        468635.48815817514,
        1.1540859241615804,
        -1.1805329947316525,
    ),
    (
        1.7391448542260006,
        0.5830662353151713,
        0.0,
        -0.25633173776953955,
        -0.29874072077210084,
    ),
    (
        1.7856404696455925,
        0.1604306598232724,
        0.0,
        -1.3559521837550683,
        -0.16043066031742897,
    ),
    (
        0.7542944088002157,
        0.026373400569765462,
        0.0,
        -2.387295718626915,
        -0.02641965406362223,
    ),
    (
        -1.5719650718809024,
        0.12220753011717533,
        0.0,
        -1.571965087346642,
        0.1222075833378568,
    ),
    (
        0.9112586747477582,
        0.7938889580187675,
        53376.86264008654,
        0.9112586761449321,
        0.7938889600536256,
    ),
];
