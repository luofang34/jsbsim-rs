// SPDX-License-Identifier: LGPL-2.1-or-later
use super::gust::{GustFrame, GustProfile};
use super::*;
use crate::fdm::Fdm;
use crate::random::RandomNumberGenerator;

fn run(w: &mut Winds) {
    w.run(false, &RandomNumberGenerator::default(), 0.0);
}

fn identity() -> Matrix33 {
    Matrix33::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0)
}

fn winds() -> Winds {
    let mut w = Winds::default();
    w.input.tl2b = identity();
    w.input.tw2b = identity();
    w.input.uvw = ColumnVector3::new(100.0, 0.0, 0.0);
    w.input.total_delta_t = 0.5;
    w
}

#[test]
fn defaults_are_the_constructor_values() {
    let w = Winds::default();
    assert_eq!(w.turbulence.turb_type, TurbulenceType::Milspec.code());
    assert_eq!(w.turbulence.gain, 1.0);
    assert_eq!(w.turbulence.rate, 10.0);
    assert_eq!(w.turbulence.rhythmicity, 0.1);
    assert_eq!(w.turbulence.severity, 0);
    let g = &w.cosine_gust;
    assert_eq!(g.frame, GustFrame::Local.code());
    assert_eq!(g.magnitude, 1.0);
    assert_eq!(
        (
            g.profile.startup_duration,
            g.profile.steady_duration,
            g.profile.end_duration
        ),
        (2.0, 4.0, 2.0)
    );
}

#[test]
fn windspeed_from_calm_blows_north_and_resets_psi() {
    let mut w = winds();
    w.psiw = 1.0;
    w.set_windspeed(20.0);
    assert_eq!(w.psiw, 0.0);
    assert_eq!(w.wind_ned, ColumnVector3::new(20.0, 0.0, 0.0));
}

#[test]
fn windspeed_keeps_direction_and_drops_the_vertical() {
    let mut w = winds();
    w.set_wind_ned(ColumnVector3::new(0.0, 10.0, 5.0));
    w.psiw = std::f64::consts::FRAC_PI_2;
    w.set_windspeed(30.0);
    assert!((w.wind_ned[E_NORTH]).abs() < 1e-12);
    assert_eq!(w.wind_ned[E_EAST], 30.0);
    assert_eq!(w.wind_ned[E_DOWN], 0.0);
}

#[test]
fn psi_keeps_the_magnitude_but_a_calm_wind_resets_it() {
    let mut w = winds();
    w.set_wind_psi(2.0);
    assert_eq!(w.psiw, 0.0);
    w.set_wind_ned(ColumnVector3::new(3.0, 4.0, 0.0));
    w.set_wind_psi(PI);
    assert_eq!(w.psiw, PI);
    assert!((w.windspeed() - 5.0).abs() < 1e-12);
    assert!((w.wind_ned[E_NORTH] + 5.0).abs() < 1e-12);
}

#[test]
fn run_sums_the_total_and_wraps_psi() {
    let mut w = winds();
    w.turbulence.turb_type = TurbulenceType::None.code();
    w.set_wind_ned(ColumnVector3::new(10.0, -10.0, 1.0));
    w.gust_ned = ColumnVector3::new(1.0, 2.0, 3.0);
    run(&mut w);
    assert_eq!(w.total_wind_ned, ColumnVector3::new(11.0, -8.0, 4.0));
    assert!((w.psiw - 1.75 * PI).abs() < 1e-12);
    // A zero north component leaves psi alone.
    w.set_wind_ned(ColumnVector3::new(0.0, 10.0, 0.0));
    run(&mut w);
    assert!((w.psiw - 1.75 * PI).abs() < 1e-12);
}

#[test]
fn holding_leaves_the_state_alone() {
    let mut w = winds();
    w.set_wind_ned(ColumnVector3::new(10.0, 0.0, 0.0));
    w.run(true, &RandomNumberGenerator::default(), 0.0);
    assert_eq!(w.total_wind_ned, ColumnVector3::zero());
}

#[test]
fn cosine_gust_rises_holds_falls_and_resets() {
    let mut w = winds();
    let g = &mut w.cosine_gust;
    g.direction = ColumnVector3::new(0.0, 0.0, -2.0);
    g.magnitude = 10.0;
    g.profile = GustProfile {
        running: true,
        elapsed_time: 0.0,
        startup_duration: 1.0,
        steady_duration: 1.0,
        end_duration: 1.0,
    };
    let mut downs = Vec::new();
    while w.cosine_gust.profile.running {
        run(&mut w);
        downs.push(w.total_wind_ned[E_DOWN]);
    }
    // Elapsed 0, 0.5, 1 (peak), 1.5, 2, 2.5; the step from 3 to 3.5 ends
    // the gust and clears it before the total is summed.
    let expected = [0.0, -5.0, -10.0, -10.0, -10.0, -5.0, 0.0];
    assert_eq!(downs.len(), expected.len());
    for (got, want) in downs.iter().zip(expected) {
        assert!((got - want).abs() < 1e-12, "{downs:?}");
    }
    assert_eq!(w.cosine_gust.direction, ColumnVector3::new(0.0, 0.0, -1.0));
    assert_eq!(w.cosine_gust.direction_local, ColumnVector3::zero());
    assert_eq!(w.cosine_gust.profile.elapsed_time, 0.0);
}

#[test]
fn body_gust_direction_is_fixed_in_the_local_frame_at_start() {
    let mut w = winds();
    // Body x points east.
    w.input.tl2b = Matrix33::new(0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    let g = &mut w.cosine_gust;
    g.frame = GustFrame::Body.code();
    g.direction = ColumnVector3::new(1.0, 0.0, 0.0);
    g.profile.running = true;
    g.profile.elapsed_time = 3.0;
    run(&mut w);
    assert_eq!(w.cosine_gust_ned, ColumnVector3::new(0.0, 1.0, 0.0));
    // Turning the aircraft does not turn the running gust.
    w.input.tl2b = identity();
    run(&mut w);
    assert_eq!(w.cosine_gust_ned, ColumnVector3::new(0.0, 1.0, 0.0));
}

#[test]
fn an_unknown_gust_frame_blows_nothing() {
    let mut w = winds();
    w.cosine_gust.frame = 7;
    w.cosine_gust.direction = ColumnVector3::new(1.0, 0.0, 0.0);
    w.cosine_gust.profile.running = true;
    w.cosine_gust.profile.elapsed_time = 3.0;
    run(&mut w);
    assert_eq!(w.cosine_gust_ned, ColumnVector3::zero());
}

#[test]
fn init_resets_gusts_but_not_the_steady_wind() {
    let mut w = winds();
    w.set_wind_ned(ColumnVector3::new(1.0, 2.0, 3.0));
    w.gust_ned = ColumnVector3::new(1.0, 1.0, 1.0);
    w.psiw = 1.0;
    w.cosine_gust.profile.running = true;
    w.cosine_gust.profile.elapsed_time = 1.0;
    w.init_model();
    assert_eq!(w.wind_ned, ColumnVector3::new(1.0, 2.0, 3.0));
    assert_eq!(w.gust_ned, ColumnVector3::zero());
    assert_eq!(w.psiw, 0.0);
    assert!(!w.cosine_gust.profile.running);
    assert_eq!(w.cosine_gust.profile.elapsed_time, 0.0);
}

#[test]
fn turbulence_free_branches_skip_the_generator() {
    let mut w = winds();
    let rng = RandomNumberGenerator::new(1);
    w.turbulence_ned = ColumnVector3::new(1.0, 1.0, 1.0);
    w.run(false, &rng, 0.0);
    assert_eq!(w.turbulence_ned, ColumnVector3::zero());
    for code in [0, 1, 7, -1] {
        w.turbulence.turb_type = code;
        w.run(false, &rng, 0.0);
    }
    w.turbulence.turb_type = TurbulenceType::Culp.code();
    w.turbulence.gain = 0.0;
    w.run(false, &rng, 0.0);
    // Severity set but no airspeed relative to the air mass.
    w.turbulence.turb_type = TurbulenceType::Tustin.code();
    w.turbulence.severity = 3;
    w.set_wind_ned(ColumnVector3::new(100.0, 0.0, 0.0));
    w.run(false, &rng, 0.0);
    assert_eq!(w.turbulence_ned, ColumnVector3::zero());
    // Nothing was drawn.
    assert_eq!(rng.uniform(), RandomNumberGenerator::new(1).uniform());
}

#[test]
fn dryden_draws_four_normals_per_frame_from_its_own_seed_once_set() {
    let mut w = winds();
    w.input.altitude_asl = 5000.0;
    w.input.wingspan = 35.0;
    w.turbulence.severity = 4;
    let exec = RandomNumberGenerator::new(1);
    w.run(false, &exec, 0.0);
    let expected = RandomNumberGenerator::new(1);
    for _ in 0..4 {
        assert!(expected.normal().is_finite());
    }
    assert_eq!(exec.normal().to_bits(), expected.normal().to_bits());
    assert_ne!(w.turbulence_ned, ColumnVector3::zero());
    // With a seed of its own, the executive's generator is left alone.
    w.set_random_seed(2);
    w.run(false, &exec, 0.0);
    assert_eq!(exec.normal().to_bits(), expected.normal().to_bits());
    w.init_model();
    assert_eq!(w.dryden, DrydenState::default());
}

#[test]
fn culp_draws_a_pulse_when_none_is_pending() {
    let mut w = winds();
    w.input.distance_agl = 1000.0;
    w.input.wingspan = 35.0;
    w.turbulence.turb_type = TurbulenceType::Culp.code();
    w.turbulence.gain = 2.0;
    let rng = RandomNumberGenerator::new(3);
    w.run(false, &rng, 1.0);
    let strength = RandomNumberGenerator::new(3).uniform();
    assert_eq!(w.culp.strength, strength);
    assert_eq!(w.culp.target_time, 1.0 + 0.71 + strength * 0.5);
    // The gain is clamped in place.
    assert_eq!(w.turbulence.gain, 1.0);
}

#[test]
fn properties_drive_the_model() {
    let mut fdm = Fdm::new().unwrap();
    fdm.set_property("atmosphere/wind-mag-fps", 10.0).unwrap();
    fdm.set_property("atmosphere/psiw-rad", PI / 2.0).unwrap();
    assert!((fdm.get_property("atmosphere/wind-east-fps").unwrap() - 10.0).abs() < 1e-12);
    fdm.set_property("atmosphere/cosine-gust/frame", 2.9)
        .unwrap();
    assert_eq!(fdm.models.winds.cosine_gust.frame, 2);
    fdm.set_property("atmosphere/cosine-gust/start", 0.5)
        .unwrap();
    assert!(fdm.models.winds.cosine_gust.profile.running);
    fdm.set_property("atmosphere/turb-type", 0.0).unwrap();
    assert_eq!(fdm.get_property("atmosphere/turb-type").unwrap(), 0.0);
    assert_eq!(fdm.get_property("atmosphere/randomseed").unwrap(), 0.0);
    fdm.set_property("simulation/randomseed", 5.0).unwrap();
    assert_eq!(fdm.get_property("atmosphere/randomseed").unwrap(), 5.0);
    fdm.set_property("atmosphere/randomseed", 9.0).unwrap();
    assert_eq!(fdm.get_property("atmosphere/randomseed").unwrap(), 9.0);
}

#[test]
fn initial_conditions_set_the_steady_wind() {
    let mut fdm = Fdm::new().unwrap();
    // No aircraft is loaded, so its aerodynamics cannot run.
    fdm.set_property("simulation/models/FGAerodynamics/enabled", 0.0)
        .unwrap();
    fdm.set_property("ic/vw-mag-fps", 5.0).unwrap();
    fdm.set_property("ic/vw-dir-deg", 30.0).unwrap();
    fdm.run_ic().unwrap();
    let mag = fdm.get_property("atmosphere/wind-mag-fps").unwrap();
    assert!((mag - 5.0).abs() < 1e-12, "{mag}");
    for axis in ["north", "east"] {
        let ic = fdm.get_property(&format!("ic/vw-{axis}-fps")).unwrap();
        assert_ne!(ic, 0.0);
        assert_eq!(
            fdm.get_property(&format!("atmosphere/total-wind-{axis}-fps"))
                .unwrap(),
            ic
        );
    }
}
