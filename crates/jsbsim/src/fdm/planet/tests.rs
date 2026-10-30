// SPDX-License-Identifier: LGPL-2.1-or-later
use crate::Error;
use crate::MemoryResolver;
use crate::fdm::Fdm;
use crate::models::atmosphere::mars::MMARS;
use crate::models::atmosphere::{AtmosphereModel, RSTAR};

const BOX: &str = r#"<fdm_config name="box">
  <metrics/>
  <mass_balance>
    <ixx>10</ixx><iyy>10</iyy><izz>10</izz><emptywt>100</emptywt>
    <location name="CG" unit="IN"><x>0</x><y>0</y><z>0</z></location>
  </mass_balance>
  <ground_reactions/>
  PLANET
</fdm_config>"#;

const MARS: &str = r#"<planet name="Mars">
  <equatorial_radius unit="KM">3396.2</equatorial_radius>
  <polar_radius unit="KM">3376.2</polar_radius>
  <rotation_rate unit="RAD/SEC">7.0882E-5</rotation_rate>
  <GM unit="M3/SEC2">4.2828372E13</GM>
  <J2>1.9605E-3</J2>
  <atmosphere model="Mars"/>
</planet>"#;

fn resolver(planet: &str) -> MemoryResolver {
    let mut r = MemoryResolver::new();
    r.insert("aircraft/box/box.xml", BOX.replace("PLANET", planet));
    r.insert("aircraft/box/mars.xml", MARS);
    r.insert("planets/mars.xml", MARS);
    r.insert("planets/moon.xml", MARS.replace("Mars", "Moon"));
    r
}

fn fdm() -> Fdm {
    let mut fdm = Fdm::new().unwrap();
    fdm.set_property("simulation/models/FGAerodynamics/enabled", 0.0)
        .unwrap();
    fdm
}

fn run_at_ground(fdm: &mut Fdm) {
    fdm.set_property("ic/h-agl-ft", 0.0).unwrap();
    fdm.set_property("ic/long-gc-deg", 0.0).unwrap();
    fdm.set_property("ic/lat-geod-deg", 0.0).unwrap();
    fdm.run_ic().unwrap();
}

fn check_mars(fdm: &mut Fdm) {
    assert_eq!(fdm.models.atmosphere.model, AtmosphereModel::Mars);
    let t = fdm.get_property("atmosphere/T-R").unwrap();
    assert!((t - 433.99).abs() < 1e-2, "{t}");
    let p = fdm.get_property("atmosphere/P-psf").unwrap();
    assert!((p - 14.62).abs() < 1e-2, "{p}");
    let rho = fdm.get_property("atmosphere/rho-slugs_ft3").unwrap();
    assert_eq!(rho, p / (RSTAR / MMARS * t));
    // `FGMars::Calculate` leaves the speed of sound at its initial value.
    let a = fdm.get_property("atmosphere/a-fps").unwrap();
    assert_eq!(a, fdm.models.atmosphere.std_day_sl_soundspeed);
}

#[test]
fn a_planet_element_with_a_file_sets_the_planet_and_its_atmosphere() {
    let r = resolver(r#"<planet file="mars"/>"#);
    let mut fdm = fdm();
    fdm.load_model(&r, "aircraft", "box").unwrap();
    run_at_ground(&mut fdm);
    let radius = fdm.get_property("metrics/terrain-radius").unwrap();
    assert!((radius * 0.3048 / 3396200.0 - 1.0).abs() < 1e-7, "{radius}");
    let g = fdm.get_property("accelerations/gravity-ft_sec2").unwrap();
    assert!((g * 0.3048 - 3.72).abs() < 5e-2, "{g}");
    check_mars(&mut fdm);
}

#[test]
fn load_planet_file_replaces_the_atmosphere_of_a_loaded_aircraft() {
    let r = resolver("");
    let mut fdm = fdm();
    fdm.load_model(&r, "aircraft", "box").unwrap();
    fdm.load_planet_file(&r, "planets/mars.xml", false).unwrap();
    run_at_ground(&mut fdm);
    check_mars(&mut fdm);
}

#[test]
fn the_standard_atmosphere_properties_are_left_untied_by_mars() {
    let r = resolver("");
    let mut fdm = fdm();
    fdm.load_model(&r, "aircraft", "box").unwrap();
    let rh = fdm.get_property("atmosphere/RH").unwrap();
    fdm.load_planet_file(&r, "planets/mars.xml", false).unwrap();
    run_at_ground(&mut fdm);
    assert!(fdm.set_property("atmosphere/delta-T", 10.0).unwrap());
    assert_eq!(fdm.get_property("atmosphere/delta-T").unwrap(), 10.0);
    fdm.run().unwrap();
    let h = fdm.models.atmosphere.input.altitude_asl;
    let t = -25.68 - 0.000548 * h + 459.67;
    assert_eq!(fdm.get_property("atmosphere/T-R").unwrap(), t);
    assert_eq!(fdm.get_property("atmosphere/RH").unwrap(), rh);
}

#[test]
fn the_atmosphere_enable_flag_stays_with_the_replaced_model() {
    let r = resolver("");
    let mut fdm = fdm();
    fdm.load_model(&r, "aircraft", "box").unwrap();
    fdm.load_planet_file(&r, "planets/mars.xml", false).unwrap();
    fdm.set_property("simulation/models/FGAtmosphere/enabled", 0.0)
        .unwrap();
    assert!(fdm.models.atmosphere.base.enabled);
    fdm.set_property("ic/h-agl-ft", 1000.0).unwrap();
    fdm.run_ic().unwrap();
    let t = fdm.get_property("atmosphere/T-R").unwrap();
    assert!(
        (t - (-25.68 - 0.000548 * 1000.0 + 459.67)).abs() < 1e-6,
        "{t}"
    );
}

#[test]
fn a_planet_radius_that_is_not_positive_is_a_geographic_error() {
    let mut r = MemoryResolver::new();
    for (name, planet) in [
        ("equatorial", MARS.replace("3396.2", "-3396.2")),
        ("polar", MARS.replace("3376.2", "-3376.2")),
    ] {
        r.insert(format!("planets/{name}.xml"), planet);
    }
    r.insert("aircraft/box/box.xml", BOX.replace("PLANET", ""));
    let mut fdm = fdm();
    fdm.load_model(&r, "aircraft", "box").unwrap();
    for (name, message) in [
        ("equatorial", "Equatorial radius is not positive"),
        ("polar", "Polar semi-axis is not positive"),
    ] {
        let err = fdm
            .load_planet_file(&r, &format!("planets/{name}.xml"), false)
            .unwrap_err();
        assert_eq!(
            err,
            Error::Geographic {
                message: message.to_owned()
            }
        );
    }
}

#[test]
fn a_file_that_is_not_a_planet_is_refused() {
    let r = resolver("");
    let mut fdm = fdm();
    assert!(matches!(
        fdm.load_planet_file(&r, "aircraft/box/box.xml", false),
        Err(Error::Model { .. })
    ));
}

const MSIS: &str = r#"<planet name="Earth">
  <atmosphere model="MSIS">
    <day>172</day>
    <utc>29000</utc>
  </atmosphere>
</planet>"#;

/// `TestPlanet.test_load_MSIS_atmosphere`.
#[test]
fn load_planet_file_with_msis_gives_the_upstream_test_values() {
    let mut r = resolver("");
    r.insert("planets/MSIS.xml", MSIS);
    let mut fdm = fdm();
    fdm.load_model(&r, "aircraft", "box").unwrap();
    fdm.load_planet_file(&r, "planets/MSIS.xml", false).unwrap();
    assert_eq!(fdm.models.atmosphere.model, AtmosphereModel::Msis);
    assert_eq!(fdm.models.atmosphere.msis.day_of_year, 172.0);
    assert_eq!(fdm.models.atmosphere.msis.seconds_in_day, 29000.0);
    fdm.set_property("ic/h-sl-ft", 0.0).unwrap();
    fdm.set_property("ic/long-gc-deg", -70.0).unwrap();
    fdm.set_property("ic/lat-geod-deg", 60.0).unwrap();
    fdm.run_ic().unwrap();
    let t = fdm.get_property("atmosphere/T-R").unwrap() * 5.0 / 9.0;
    assert!((t - 281.46476).abs() < 1e-5, "{t}");
    let rho = fdm.get_property("atmosphere/rho-slugs_ft3").unwrap() / 0.001940318;
    assert!((rho - 1.263428).abs() < 1e-6, "{rho}");
    let p = fdm.get_property("atmosphere/P-psf").unwrap();
    assert!((p - 2132.294).abs() < 1e-3, "{p}");
    // FGMSIS is a standard atmosphere: its own properties stay bound.
    let sl = fdm.models.atmosphere.sl_pressure;
    assert_eq!(fdm.get_property("atmosphere/P-sl-psf").unwrap(), sl);
}
