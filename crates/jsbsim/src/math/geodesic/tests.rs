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
