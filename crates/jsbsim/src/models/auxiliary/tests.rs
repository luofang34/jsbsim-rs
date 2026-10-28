// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use crate::fdm::Fdm;
use crate::models::atmosphere::RENG0;
use crate::models::atmosphere::SH_RATIO;
use crate::models::inertial::STANDARD_GRAVITY;

const A_WGS84: f64 = 20925646.32546;
const B_WGS84: f64 = 20855486.5951;

fn sea_level() -> Auxiliary {
    let mut x = Auxiliary::default();
    let i = &mut x.input;
    let sound_speed = (SH_RATIO * RENG0 * STD_DAY_SL_TEMPERATURE).sqrt();
    i.air = Air {
        pressure: STD_DAY_SL_PRESSURE,
        density: STD_DAY_SL_DENSITY,
        temperature: STD_DAY_SL_TEMPERATURE,
        sound_speed,
        kinematic_viscosity: 1.57e-4,
    };
    i.constants = Constants {
        std_day_sl_soundspeed: sound_speed,
        standard_gravity: STANDARD_GRAVITY,
        wingspan: 30.0,
        wingchord: 5.0,
    };
    i.mass = 100.0;
    i.distance_agl = 60.0;
    i.cos_tht = 1.0;
    i.cos_phi = 1.0;
    i.tl2b = identity();
    i.tb2l = identity();
    x
}

fn identity() -> Matrix33 {
    Matrix33::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0)
}

#[test]
fn construction_starts_at_standard_day_total_conditions() {
    let x = Auxiliary::default();
    assert_eq!(x.airspeed.pt, STD_DAY_SL_PRESSURE);
    assert_eq!(x.airspeed.tat, STD_DAY_SL_TEMPERATURE);
    assert_eq!(x.airspeed.tatc, rankine_to_celsius(STD_DAY_SL_TEMPERATURE));
}

#[test]
fn at_rest_everything_is_zero_and_the_wind_axes_are_the_body_axes() {
    let mut x = sea_level();
    x.run(false);
    assert_eq!((x.vt, x.alpha, x.beta, x.qbar), (0.0, 0.0, 0.0, 0.0));
    assert_eq!(x.airspeed.mach, 0.0);
    assert_eq!((x.airspeed.vcas, x.airspeed.veas), (0.0, 0.0));
    assert_eq!(x.tw2b, identity());
    assert_eq!(x.tb2w, identity());
    assert_eq!(x.airspeed.pt, STD_DAY_SL_PRESSURE);
    assert_eq!(x.hoverbcg, 2.0);
}

#[test]
fn angles_follow_the_aero_relative_velocity() {
    let mut x = sea_level();
    x.input.uvw = ColumnVector3::new(100.0, 5.0, 10.0);
    x.input.uvwdot = ColumnVector3::new(1.0, 0.5, 2.0);
    x.run(false);
    assert_eq!(x.alpha, libm::atan2(10.0, 100.0));
    assert_eq!(x.beta, libm::atan2(5.0, (100.0f64 * 100.0 + 100.0).sqrt()));
    assert_eq!(x.adot, (100.0 * 2.0 - 10.0 * 1.0) / 10100.0);
    assert!(x.bdot != 0.0);
    assert_eq!(x.qbar, 0.5 * STD_DAY_SL_DENSITY * 10125.0);
    assert_eq!(x.tw2b.transposed(), x.tb2w);
}

#[test]
fn pure_sideslip_has_no_alpha_and_tiny_speed_has_no_angles() {
    let mut x = sea_level();
    x.input.uvw = ColumnVector3::new(0.0, 10.0, 0.0);
    x.input.uvwdot = ColumnVector3::new(1.0, 1.0, 1.0);
    x.run(false);
    assert_eq!(x.beta, std::f64::consts::FRAC_PI_2);
    assert_eq!((x.alpha, x.adot, x.bdot), (0.0, 0.0, 0.0));

    x.input.uvw = ColumnVector3::new(0.0005, 0.0, 0.0);
    x.run(false);
    assert_eq!((x.alpha, x.beta), (0.0, 0.0));
    assert_eq!(x.vt, 0.0005);
}

#[test]
fn wind_subtracts_from_the_body_velocity() {
    let mut x = sea_level();
    x.input.total_wind_ned = ColumnVector3::new(-20.0, 0.0, 0.0);
    x.input.turb_pqr = ColumnVector3::new(0.1, 0.0, 0.0);
    x.run(false);
    assert_eq!(x.aero_uvw, ColumnVector3::new(20.0, 0.0, 0.0));
    assert_eq!(x.aero_pqr, ColumnVector3::new(-0.1, 0.0, 0.0));
    assert_eq!(x.vt, 20.0);
    assert_eq!(x.airspeed.vground, 0.0);
}

/// Above Mach 1 the ten fixed iterations of equation 4.17 stop a few
/// parts per billion short of convergence.
#[test]
fn calibrated_equals_true_airspeed_on_a_standard_day_at_sea_level() {
    for u in [200.0, 1500.0] {
        let mut x = sea_level();
        x.input.uvw = ColumnVector3::new(u, 0.0, 0.0);
        x.run(false);
        let s = x.airspeed;
        assert!((s.vcas - u).abs() < 1e-8 * u, "{u}: vcas {}", s.vcas);
        assert!((s.veas - u).abs() < 1e-9 * u, "{u}: veas {}", s.veas);
        let mach = x.mach_from_vcalibrated(s.vcas, STD_DAY_SL_PRESSURE);
        assert!((mach - s.mach).abs() < 1e-8);
    }
}

#[test]
fn pitot_pressure_is_continuous_at_mach_one_and_static_for_negative_mach() {
    let p = STD_DAY_SL_PRESSURE;
    let below = pitot_total_pressure(1.0 - 1e-12, p);
    let at = pitot_total_pressure(1.0, p);
    assert!((below - at).abs() < 1e-8 * p);
    assert_eq!(pitot_total_pressure(-0.5, p), p);
    let qc = pitot_total_pressure(2.0, p) - p;
    assert!((mach_from_impact_pressure(qc, p) - 2.0).abs() < 1e-8);
}

#[test]
fn euler_rates_keep_psi_and_phi_at_vertical_pitch() {
    let mut x = sea_level();
    x.input.pqr = ColumnVector3::new(0.1, 0.2, 0.3);
    x.run(false);
    let before = x.euler_rates;
    x.input.cos_tht = 0.0;
    x.input.sin_tht = 1.0;
    x.input.pqr = ColumnVector3::new(1.0, 2.0, 3.0);
    x.run(false);
    assert_eq!(x.euler_rates[E_PHI], before[E_PHI]);
    assert_eq!(x.euler_rates[E_PSI], before[E_PSI]);
    assert_eq!(x.euler_rates[E_THT], 2.0);
}

#[test]
fn init_model_keeps_nx_and_resets_ny_nz() {
    let mut x = sea_level();
    x.input.body_accel = ColumnVector3::new(STANDARD_GRAVITY, STANDARD_GRAVITY, STANDARD_GRAVITY);
    x.run(false);
    let l = x.loads;
    assert_eq!((l.nx, l.ny, l.nz), (1.0, 1.0, -1.0));
    x.init_model();
    let l = x.loads;
    assert_eq!((l.nx, l.ny, l.nz), (1.0, 0.0, 0.0));
}

#[test]
fn holding_or_a_disabled_model_does_not_run() {
    let mut x = sea_level();
    x.input.uvw = ColumnVector3::new(100.0, 0.0, 0.0);
    x.run(true);
    assert_eq!(x.vt, 0.0);
    x.base.enabled = false;
    x.run(false);
    assert_eq!(x.vt, 0.0);
}

#[test]
fn load_factor_is_zero_without_mass() {
    let mut x = sea_level();
    x.input.fw = ColumnVector3::new(0.0, 0.0, 3217.4049);
    assert_eq!(x.nlf(), 3217.4049 / (100.0 * crate::units::SLUGTOLB));
    x.input.mass = 0.0;
    assert_eq!(x.nlf(), 0.0);
}

#[test]
fn distances_from_start_vanish_at_the_start_and_grow_with_offset() {
    let mut x = sea_level();
    let mut here = Location::default();
    here.set_ellipse(A_WGS84, B_WGS84);
    here.set_position_geodetic(0.1, 0.5, 1000.0);
    x.input.location = here;
    x.set_initial_state(&here, A_WGS84, B_WGS84);
    let (lon, lat) = (here.longitude(), here.geod_latitude_rad());
    assert_eq!(x.distance_relative_position(lon, lat), 0.0);
    let neu = x.neu_position_from_start();
    assert!(neu[E_X].abs() < 1e-6 && neu[E_Y].abs() < 1e-6);
    assert!((neu[E_Z] - 1000.0).abs() < 1e-6);

    let north = x.latitude_relative_position(lat + 1e-4);
    assert!((north - 1e-4 * 6.36e6).abs() < 0.01 * 6.36e2, "{north}");
    assert_eq!(x.longitude_relative_position(lon), 0.0);
    assert!(here.heading_to(0.1, 0.5 + 1e-4).abs() < 1e-9);
}

#[test]
fn properties_read_the_computed_values() {
    let mut fdm = Fdm::new().unwrap();
    let m = &mut fdm.models;
    let c = m.auxiliary.input.constants;
    assert_eq!(c.standard_gravity, STANDARD_GRAVITY);
    assert_eq!(c.std_day_sl_soundspeed, m.atmosphere.std_day_sl_soundspeed);
    m.auxiliary.input = sea_level().input;
    m.auxiliary.input.uvw = ColumnVector3::new(300.0, 0.0, 30.0);
    m.auxiliary.run(false);
    let alpha = fdm.models.auxiliary.alpha;
    assert_eq!(fdm.get_property("aero/alpha-rad").unwrap(), alpha);
    assert_eq!(
        fdm.get_property("aero/alpha-deg").unwrap(),
        alpha * crate::units::RADTODEG
    );
    let vt = fdm.models.auxiliary.vt;
    assert_eq!(fdm.get_property("velocities/vt-fps").unwrap(), vt);
    assert_eq!(fdm.get_property("velocities/u-aero-fps").unwrap(), 300.0);
    assert!(fdm.get_property("velocities/vc-kts").unwrap() > 0.0);
}

#[test]
fn run_ic_measures_from_the_initial_position() {
    let mut fdm = Fdm::new().unwrap();
    fdm.set_property("ic/long-gc-rad", 0.1).unwrap();
    fdm.set_property("ic/lat-geod-rad", 0.5).unwrap();
    fdm.set_property("ic/h-sl-ft", 1000.0).unwrap();
    // No aircraft is loaded, so there is no aerodynamics to run.
    fdm.models.aerodynamics.base.enabled = false;
    fdm.run_ic().unwrap();
    let mag = fdm
        .get_property("position/distance-from-start-mag-mt")
        .unwrap();
    assert!(mag.abs() < 1e-6, "{mag}");
    let up = fdm.get_property("position/from-start-neu-u-ft").unwrap();
    let height = fdm.get_property("ic/geod-alt-ft").unwrap();
    assert!((up - height).abs() < 1e-6, "{up} {height}");
    // JSBSim measures from the live initial conditions, not a copy.
    fdm.set_property("ic/lat-geod-rad", 0.5 + 1e-4).unwrap();
    let north = fdm
        .get_property("position/distance-from-start-lat-mt")
        .unwrap();
    assert!((north - 1e-4 * 6.36e6).abs() < 0.01 * 6.36e2, "{north}");
}
