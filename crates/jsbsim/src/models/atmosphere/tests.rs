// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use crate::fdm::Fdm;

fn run_at(fdm: &mut Fdm, altitude: f64) {
    fdm.models.atmosphere.input.altitude_asl = altitude;
    fdm.run_atmosphere(false).unwrap();
}

#[test]
fn sea_level_is_the_standard_day() {
    let mut fdm = Fdm::new().unwrap();
    run_at(&mut fdm, 0.0);
    let a = &fdm.models.atmosphere;
    assert_eq!(a.temperature, STD_DAY_SL_TEMPERATURE);
    assert_eq!(a.pressure, STD_DAY_SL_PRESSURE);
    assert!((a.density - 0.0023769).abs() < 1e-7, "{}", a.density);
    assert!((a.sound_speed - 1116.45).abs() < 0.01, "{}", a.sound_speed);
    assert_eq!(a.get_temperature_ratio(), 1.0);
    assert!(a.pressure_altitude.abs() < 1e-9);
    assert_eq!(fdm.get_property("atmosphere/delta").unwrap(), 1.0);
}

#[test]
fn tropopause_temperature_comes_from_the_table() {
    let fdm = Fdm::new().unwrap();
    let a = &fdm.models.atmosphere;
    let h = a.geometric_altitude(36089.2388);
    assert!((a.get_temperature(h) - 389.97).abs() < 1e-9);
    // The stratosphere is isothermal.
    assert!((a.get_temperature(h + 5000.0) - 389.97).abs() < 1e-9);
    assert!(a.get_pressure(h + 5000.0) < a.get_pressure(h));
}

#[test]
fn bias_shifts_temperature_but_not_sea_level_pressure() {
    let mut fdm = Fdm::new().unwrap();
    assert!(fdm.set_property("atmosphere/delta-T", 18.0).unwrap());
    run_at(&mut fdm, 10000.0);
    let a = &fdm.models.atmosphere;
    assert_eq!(a.get_temperature_sl(), STD_DAY_SL_TEMPERATURE + 18.0);
    assert_eq!(a.get_std_temperature(10000.0) + 18.0, a.temperature);
    assert_eq!(a.get_pressure(0.0), STD_DAY_SL_PRESSURE);
    assert_eq!(fdm.get_property("atmosphere/delta-T").unwrap(), 18.0);
}

#[test]
fn bias_below_absolute_zero_is_capped() {
    let mut fdm = Fdm::new().unwrap();
    fdm.set_property("atmosphere/delta-T", -1000.0).unwrap();
    let a = &fdm.models.atmosphere;
    let min = a.profile.temperature_table.min_value();
    assert_eq!(a.get_temperature_bias(TemperatureUnit::Rankine), 1.8 - min);
}

#[test]
fn sea_level_pressure_rescales_the_profile() {
    let mut fdm = Fdm::new().unwrap();
    fdm.set_property("atmosphere/P-sl-psf", 2000.0).unwrap();
    run_at(&mut fdm, 0.0);
    let a = &fdm.models.atmosphere;
    assert_eq!(a.pressure, 2000.0);
    assert!(a.pressure_altitude > 1000.0);
    fdm.models.atmosphere.reset_sl_pressure();
    assert_eq!(
        fdm.models.atmosphere.get_pressure_sl(PressureUnit::Psf),
        STD_DAY_SL_PRESSURE
    );
}

#[test]
fn unit_conversions_round_trip() {
    for unit in [
        TemperatureUnit::Fahrenheit,
        TemperatureUnit::Celsius,
        TemperatureUnit::Rankine,
        TemperatureUnit::Kelvin,
    ] {
        let r = Atmosphere::convert_to_rankine(15.0, unit);
        assert!((Atmosphere::convert_from_rankine(r, unit) - 15.0).abs() < 1e-12);
    }
    assert_eq!(
        Atmosphere::convert_to_rankine(15.0, TemperatureUnit::Celsius),
        518.67
    );
    let psf = Atmosphere::convert_to_psf(1013.25, PressureUnit::Millibars);
    assert!((psf - STD_DAY_SL_PRESSURE).abs() < 0.05);
}

#[test]
fn overrides_replace_modeled_values() {
    let mut fdm = Fdm::new().unwrap();
    fdm.set_property("atmosphere/override/temperature", 500.0)
        .unwrap();
    fdm.set_property("atmosphere/override/density", 0.002)
        .unwrap();
    run_at(&mut fdm, 5000.0);
    let a = &fdm.models.atmosphere;
    assert_eq!(a.temperature, 500.0);
    assert_eq!(a.density, 0.002);
    assert_eq!(a.pressure, a.get_pressure(5000.0));
}

#[test]
fn holding_and_disabled_models_do_not_calculate() {
    let mut fdm = Fdm::new().unwrap();
    fdm.models.atmosphere.input.altitude_asl = 20000.0;
    fdm.run_atmosphere(true).unwrap();
    assert_eq!(fdm.models.atmosphere.temperature, STD_DAY_SL_TEMPERATURE);
    fdm.set_property("simulation/models/FGAtmosphere/enabled", 0.0)
        .unwrap();
    fdm.run_atmosphere(false).unwrap();
    assert_eq!(fdm.models.atmosphere.temperature, STD_DAY_SL_TEMPERATURE);
}

#[test]
fn humidity_is_capped_by_saturation() {
    let mut fdm = Fdm::new().unwrap();
    run_at(&mut fdm, 0.0);
    fdm.set_property("atmosphere/RH", 150.0).unwrap();
    let rh = fdm.get_property("atmosphere/RH").unwrap();
    assert!((rh - 100.0).abs() < 1e-9, "{rh}");
    assert!(fdm.models.atmosphere.reng > RENG0);
    let dew = fdm.get_property("atmosphere/dew-point-R").unwrap();
    assert!((dew - STD_DAY_SL_TEMPERATURE).abs() < 1e-6, "{dew}");
}

#[test]
fn standard_table_prints_every_thousand_feet() {
    let mut fdm = Fdm::new().unwrap();
    let text = fdm
        .models
        .atmosphere
        .print_standard_atmosphere_table(&Overrides::default());
    assert_eq!(text.lines().count(), 2 + 280);
    assert_eq!(
        text.lines().nth(2).unwrap(),
        "           0         59           2116        0.0023769118"
    );
}
