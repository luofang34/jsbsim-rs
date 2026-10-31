// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use crate::fdm::Fdm;
use crate::math::{E_PHI, E_PSI, E_THT, E_X, E_Y, E_Z};
use crate::models::auxiliary::{mach_from_vcalibrated, vcalibrated_from_mach};
use crate::units::DEGTORAD;
use crate::{Error, MemoryResolver};

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * (1.0 + b.abs())
}

fn fdm() -> Fdm {
    let mut fdm = Fdm::new().unwrap();
    // No aircraft is loaded, so the aerodynamics would refuse to run.
    fdm.set_property("simulation/models/FGAerodynamics/enabled", 0.0)
        .unwrap();
    fdm
}

fn load(fdm: &mut Fdm, xml: &str) -> Result<bool, Error> {
    let mut resolver = MemoryResolver::new();
    resolver.insert("aircraft/x/init.xml", xml.as_bytes().to_vec());
    fdm.load_initial_conditions(&resolver, "aircraft/x", "init")
}

#[test]
fn starts_at_rest_on_the_equator_at_sea_level() {
    let mut fdm = fdm();
    for (path, value) in [
        ("ic/h-sl-ft", 0.0),
        ("ic/lat-gc-deg", 0.0),
        ("ic/vt-fps", 0.0),
        ("ic/targetNlf", 1.0),
    ] {
        assert!(
            close(fdm.get_property(path).unwrap(), value, 1e-9),
            "{path}"
        );
    }
    assert_eq!(fdm.models.ic.speed_set(), SpeedSet::Vt);
}

#[test]
fn calibrated_and_mach_speeds_are_kept_across_altitude_changes() {
    let mut fdm = fdm();
    fdm.set_property("ic/vc-kts", 200.0).unwrap();
    let vt_sl = fdm.get_property("ic/vt-kts").unwrap();
    fdm.set_property("ic/h-sl-ft", 20000.0).unwrap();
    assert!(close(fdm.get_property("ic/vc-kts").unwrap(), 200.0, 1e-9));
    assert!(fdm.get_property("ic/vt-kts").unwrap() > vt_sl * 1.2);

    fdm.set_property("ic/mach", 0.6).unwrap();
    fdm.set_property("ic/h-agl-ft", 5000.0).unwrap();
    assert!(close(fdm.get_property("ic/mach").unwrap(), 0.6, 1e-12));
    assert!(close(
        fdm.get_property("ic/h-agl-ft").unwrap(),
        5000.0,
        1e-9
    ));

    fdm.set_property("ic/vt-fps", 300.0).unwrap();
    fdm.set_property("ic/h-sl-ft", 1000.0).unwrap();
    assert_eq!(fdm.get_property("ic/vt-fps").unwrap(), 300.0);
}

#[test]
fn flight_path_and_alpha_solve_for_theta() {
    let mut fdm = fdm();
    fdm.set_property("ic/vt-fps", 300.0).unwrap();
    fdm.set_property("ic/gamma-deg", 5.0).unwrap();
    let roc = fdm.get_property("ic/roc-fps").unwrap();
    assert!(close(roc, 300.0 * libm::sin(5.0 * DEGTORAD), 1e-12));
    fdm.set_property("ic/alpha-deg", 4.0).unwrap();
    assert!(close(fdm.get_property("ic/gamma-deg").unwrap(), 5.0, 1e-9));
    assert!(close(fdm.get_property("ic/theta-deg").unwrap(), 9.0, 1e-9));
    assert!(close(
        fdm.get_property("ic/w-fps").unwrap(),
        300.0 * libm::sin(4.0 * DEGTORAD),
        1e-9
    ));
    // A climb rate above the airspeed is refused.
    fdm.set_property("ic/roc-fps", 400.0).unwrap();
    assert!(close(fdm.get_property("ic/roc-fps").unwrap(), roc, 1e-9));
}

#[test]
fn wind_keeps_the_ground_velocity() {
    let mut fdm = fdm();
    fdm.set_property("ic/psi-true-deg", 90.0).unwrap();
    fdm.set_property("ic/vt-kts", 100.0).unwrap();
    let ve = fdm.get_property("ic/ve-fps").unwrap();
    fdm.models.ic.set_head_wind_kts(20.0);
    assert_eq!(fdm.get_property("ic/ve-fps").unwrap(), ve);
    // JSBSim adds this "wind" to the ground velocity to get the airspeed,
    // the opposite sign of FGAuxiliary, so a head wind slows the airspeed.
    assert!(close(fdm.get_property("ic/vt-kts").unwrap(), 80.0, 1e-12));
    assert!(close(
        fdm.get_property("ic/vw-mag-fps").unwrap(),
        20.0 * KTS,
        1e-12
    ));
    assert!(close(
        fdm.get_property("ic/vw-dir-deg").unwrap(),
        -90.0,
        1e-12
    ));
    assert!(close(
        fdm.get_property("ic/vw-bx-fps").unwrap(),
        -20.0 * KTS,
        1e-12
    ));
    assert!(!fdm.set_property("ic/vw-bx-fps", 1.0).unwrap(), "read-only");
}

const KTS: f64 = crate::units::KTSTOFPS;

#[test]
fn euler_angles_keep_the_body_velocity() {
    let mut ic = InitialCondition::new(&Inertial::default());
    ic.set_body_vel_fps(E_X, 100.0);
    ic.set_euler_angle_deg(E_THT, 10.0);
    assert!(close(ic.body_vel_fps(E_X), 100.0, 1e-12));
    assert!(close(
        ic.climb_rate_fps(),
        100.0 * libm::sin(10.0 * DEGTORAD),
        1e-12
    ));
    ic.set_ned_vel_fps(E_X, 50.0);
    ic.set_euler_angle_deg(E_PSI, 90.0);
    assert_eq!(
        ic.ned_vel_fps(E_X),
        50.0,
        "a local velocity set last is kept"
    );
    assert!(close(ic.euler_deg(E_PHI), 0.0, 1e-12));
}

#[test]
fn version_1_files_load_through_the_setters() {
    let mut fdm = fdm();
    let loaded = load(
        &mut fdm,
        r#"<initialize name="t">
             <ubody unit="FT/SEC"> 500.0 </ubody>
             <latitude unit="DEG"> 30.0 </latitude>
             <longitude unit="DEG"> 90.0 </longitude>
             <theta unit="DEG"> 40.0 </theta>
             <altitude unit="FT"> 10000.0 </altitude>
             <trim> 0 </trim>
           </initialize>"#,
    );
    assert_eq!(loaded, Ok(true));
    assert!(close(fdm.get_property("ic/u-fps").unwrap(), 500.0, 1e-12));
    assert!(close(
        fdm.get_property("ic/lat-gc-deg").unwrap(),
        30.0,
        1e-12
    ));
    assert!(close(
        fdm.get_property("ic/long-gc-deg").unwrap(),
        90.0,
        1e-12
    ));
    assert!(close(
        fdm.get_property("ic/h-agl-ft").unwrap(),
        10000.0,
        1e-9
    ));
    assert!(close(
        fdm.get_property("ic/theta-deg").unwrap(),
        40.0,
        1e-12
    ));
}

#[test]
fn version_2_files_load_frames() {
    let mut fdm = fdm();
    let loaded = load(
        &mut fdm,
        r#"<initialize name="t" version="2.0">
             <position frame="ECEF"><altitudeMSL unit="FT"> 800000.0 </altitudeMSL></position>
             <orientation unit="DEG" frame="LOCAL"><yaw> 90.0 </yaw></orientation>
             <velocity unit="FT/SEC" frame="BODY"><x> 23869.9 </x></velocity>
             <attitude_rate unit="DEG/SEC" frame="ECI"><z> 0.0 </z></attitude_rate>
           </initialize>"#,
    );
    assert_eq!(loaded, Ok(true));
    assert!(close(
        fdm.get_property("ic/h-sl-ft").unwrap(),
        800000.0,
        1e-9
    ));
    assert!(close(
        fdm.get_property("ic/psi-true-deg").unwrap(),
        90.0,
        1e-12
    ));
    assert!(close(
        fdm.get_property("ic/ve-fps").unwrap(),
        23869.9,
        1e-12
    ));
    let omega = fdm.models.inertial.omega_planet[E_Z];
    // At rest in ECI, the body turns against the planet's rotation.
    assert!(close(fdm.models.ic.pqr_radps().magnitude(), omega, 1e-9));
    assert_eq!(fdm.models.ic.speed_set(), SpeedSet::Uvw);
}

#[test]
fn running_engines_are_recorded_by_number() {
    let cases: [(&str, &[usize], &[usize]); 3] = [
        ("<running> -1 </running>", &[0, 1, 31], &[]),
        (
            "<running> 0 </running><running>2</running>",
            &[0, 2],
            &[1, 3],
        ),
        ("", &[], &[0]),
    ];
    for (body, running, stopped) in cases {
        let mut fdm = fdm();
        let xml = format!("<initialize><vt unit=\"FT/SEC\">10</vt>{body}</initialize>");
        assert_eq!(load(&mut fdm, &xml), Ok(true), "{body}");
        for &n in running {
            assert!(fdm.models.ic.is_engine_running(n), "{body}: {n}");
        }
        for &n in stopped {
            assert!(!fdm.models.ic.is_engine_running(n), "{body}: {n}");
        }
        assert!(!fdm.models.ic.is_engine_running(32));
    }
}

#[test]
fn a_version_1_trim_is_recorded_as_jsbsim_spells_it() {
    use crate::trim::TrimMode;
    for (body, mode) in [
        ("<trim> Longitudinal </trim>", TrimMode::Longitudinal),
        ("<trim>sideways</trim>", TrimMode::None),
        ("<trim>longitudinal</trim>", TrimMode::Longitudinal),
        ("<trim>1</trim>", TrimMode::Ground),
        ("<trim>FULL</trim>", TrimMode::Full),
        ("<trim>turn</trim>", TrimMode::Turn),
        ("<trim>0</trim>", TrimMode::None),
    ] {
        let mut fdm = fdm();
        let xml = format!("<initialize><vt unit=\"FT/SEC\">10</vt>{body}</initialize>");
        assert_eq!(load(&mut fdm, &xml), Ok(true), "{body}");
        assert_eq!(fdm.models.ic.trim_requested(), mode, "{body}");
        let vt = fdm.get_property("ic/vt-fps").unwrap();
        assert!((vt - 10.0).abs() < 1e-9, "{body}: {vt}");
    }
}

#[test]
fn inconsistent_files_fail_as_in_jsbsim() {
    let mut fdm = fdm();
    let bad_latitude = r#"<initialize><latitude unit="DEG"> 95 </latitude></initialize>"#;
    assert_eq!(load(&mut fdm, bad_latitude), Ok(false));
    let no_position = r#"<initialize version="2"/>"#;
    assert_eq!(load(&mut fdm, no_position), Ok(false));
    for xml in [r#"<initialize version="3"/>"#, "<reset/>"] {
        assert!(
            matches!(load(&mut fdm, xml), Err(Error::Model { .. })),
            "{xml}"
        );
    }
}

#[test]
fn run_ic_applies_the_live_initial_conditions() {
    let mut fdm = fdm();
    fdm.set_property("ic/h-sl-ft", 3000.0).unwrap();
    fdm.set_property("ic/u-fps", 150.0).unwrap();
    fdm.run_ic().unwrap();
    assert!(close(
        fdm.get_property("position/h-sl-ft").unwrap(),
        3000.0,
        1e-9
    ));
    assert!(close(
        fdm.get_property("velocities/u-fps").unwrap(),
        150.0,
        1e-9
    ));

    fdm.set_property("ic/h-sl-ft", 6000.0).unwrap();
    fdm.reset_to_initial_conditions(crate::ResetMask::empty())
        .unwrap();
    assert!(close(
        fdm.get_property("position/h-sl-ft").unwrap(),
        6000.0,
        1e-9
    ));
}

#[test]
fn a_reset_without_run_ic_matches_simulation_reset_2() {
    use crate::ResetMask;
    let started = || {
        let mut fdm = fdm();
        fdm.set_property("ic/h-sl-ft", 3000.0).unwrap();
        fdm.run_ic().unwrap();
        for _ in 0..5 {
            fdm.run().unwrap();
        }
        fdm.set_property("ic/h-sl-ft", 6000.0).unwrap();
        fdm
    };
    let mut typed = started();
    typed
        .reset_to_initial_conditions(ResetMask::DONT_EXECUTE_RUN_IC)
        .unwrap();
    let mut raw = started();
    raw.set_property("simulation/reset", 2.0).unwrap();
    for p in [
        "position/h-sl-ft",
        "velocities/u-fps",
        "simulation/sim-time-sec",
        "simulation/frame",
    ] {
        let (t, r) = (typed.get_property(p).unwrap(), raw.get_property(p).unwrap());
        assert_eq!(t.to_bits(), r.to_bits(), "{p}: {t} vs {r}");
    }
    assert_eq!(typed.get_property("simulation/sim-time-sec").unwrap(), 0.0);
    assert!(!close(
        typed.get_property("position/h-sl-ft").unwrap(),
        6000.0,
        1.0
    ));

    let mut full = started();
    full.reset_to_initial_conditions(ResetMask::empty())
        .unwrap();
    assert!(close(
        full.get_property("position/h-sl-ft").unwrap(),
        6000.0,
        1e-9
    ));
}

#[test]
fn two_executives_have_independent_initial_conditions() {
    let mut a = fdm();
    let mut b = fdm();
    a.set_property("ic/vt-fps", 100.0).unwrap();
    b.set_property("ic/vt-fps", 200.0).unwrap();
    assert_eq!(a.get_property("ic/vt-fps").unwrap(), 100.0);
    assert_eq!(b.get_property("ic/vt-fps").unwrap(), 200.0);
}

#[test]
fn terrain_elevation_keeps_a_height_above_ground() {
    let mut fdm = fdm();
    fdm.set_property("ic/h-agl-ft", 500.0).unwrap();
    fdm.set_property("ic/terrain-elevation-ft", 1000.0).unwrap();
    assert!(close(
        fdm.get_property("ic/terrain-elevation-ft").unwrap(),
        1000.0,
        1e-9
    ));
    assert!(close(fdm.get_property("ic/h-agl-ft").unwrap(), 500.0, 1e-9));
    assert!(close(fdm.get_property("ic/h-sl-ft").unwrap(), 1500.0, 1e-9));
}

/// JSBSim's initial conditions convert calibrated airspeed through
/// `FGAuxiliary`, whose speed of sound is the atmosphere's; the port's
/// initial conditions pass the atmosphere's directly, so the two must
/// agree.
#[test]
fn initial_conditions_convert_with_the_auxiliary_speed_of_sound() {
    let fdm = fdm();
    let m = &fdm.models;
    assert_eq!(
        m.atmosphere.std_day_sl_soundspeed.to_bits(),
        m.auxiliary.input.constants.std_day_sl_soundspeed.to_bits()
    );
    let (mach, p) = (0.8, 700.0);
    let sl = m.atmosphere.std_day_sl_soundspeed;
    assert_eq!(
        vcalibrated_from_mach(mach, p, sl).to_bits(),
        m.auxiliary.vcalibrated_from_mach(mach, p).to_bits()
    );
}

#[test]
fn pitot_relations_invert_each_other() {
    let sl = crate::models::atmosphere::Atmosphere::default().std_day_sl_soundspeed;
    for mach in [0.3, 0.9, 1.5, 2.5] {
        let p = 800.0;
        let vc = vcalibrated_from_mach(mach, p, sl);
        assert!(
            close(mach_from_vcalibrated(vc, p, sl), mach, 1e-9),
            "{mach}"
        );
    }
}

#[test]
fn reset_ic_sets_body_state_then_the_flight_path() {
    let atmosphere = crate::models::atmosphere::Atmosphere::default();
    let inertial = Inertial::default();
    let env = Environment::new(&atmosphere, &inertial);
    let mut ic = InitialCondition::new(&inertial);
    ic.reset_ic(
        env,
        ColumnVector3::new(200.0, 0.0, 0.0),
        ColumnVector3::new(0.1, 0.2, 0.3),
        0.0,
        0.0,
        ColumnVector3::new(0.0, 0.0, 0.5),
        0.3,
        -1.0,
        1000.0,
        2.0 * DEGTORAD,
    );
    assert!(close(ic.altitude_agl_ft(env), 1000.0, 1e-9));
    // The altitude is applied along the geodetic vertical, which moves the
    // geocentric latitude slightly, as in JSBSim.
    assert!(close(ic.latitude_rad(), 0.3, 1e-4));
    assert!(ic.latitude_rad() != 0.3);
    assert_eq!(ic.pqr_radps(), ColumnVector3::new(0.1, 0.2, 0.3));
    assert!(close(ic.vtrue_fps(), 200.0, 1e-12));
    assert!(close(ic.flight_path_angle_deg(), 2.0, 1e-9));
    assert!(close(ic.euler_rad(E_PSI), 0.5, 1e-12));
}

#[test]
fn wind_components_add_to_the_ground_velocity() {
    let mut ic = InitialCondition::new(&Inertial::default());
    ic.set_ned_vel_fps(E_X, 100.0);
    ic.set_wind_ned_fps(10.0, -20.0, 0.0);
    assert!(close(
        ic.vtrue_fps(),
        (110.0f64 * 110.0 + 400.0).sqrt(),
        1e-12
    ));
    let w = ic.wind_ned_fps();
    assert!(close(w[E_X], 10.0, 1e-12) && close(w[E_Y], -20.0, 1e-12));
    ic.set_wind_down_kts(5.0);
    assert!(close(ic.wind_ned_fps()[E_Z], 5.0 * KTS, 1e-12));
    assert_eq!(ic.ned_vel_fps(E_X), 100.0, "the ground velocity is kept");
}

/// `load_initial_conditions_file` reads the file at a path, `.xml` added
/// when it has no extension, a file at the resolver's root included,
/// where `load_initial_conditions` with no directory reads `/name`.
#[test]
fn an_initialization_file_loads_by_its_path() {
    let xml = r#"<initialize name="t">
             <ubody unit="FT/SEC"> 500.0 </ubody>
             <latitude unit="DEG"> 30.0 </latitude>
             <longitude unit="DEG"> 90.0 </longitude>
             <theta unit="DEG"> 40.0 </theta>
             <altitude unit="FT"> 1234.0 </altitude>
             <trim> 0 </trim>
           </initialize>"#;
    let mut resolver = MemoryResolver::new();
    resolver.insert("reset00.xml", xml);
    resolver.insert("scenarios/takeoff.xml", xml);
    for path in [
        "reset00",
        "reset00.xml",
        "scenarios/takeoff",
        "scenarios/./takeoff.xml",
    ] {
        let mut fdm = fdm();
        assert!(
            fdm.load_initial_conditions_file(&resolver, path).unwrap(),
            "{path}"
        );
        assert!(
            close(fdm.get_property("ic/h-agl-ft").unwrap(), 1234.0, 1e-9),
            "{path}"
        );
    }
    let rooted = fdm().load_initial_conditions(&resolver, "", "reset00");
    assert!(
        matches!(&rooted, Err(Error::ResourceNotFound { path }) if path == "/reset00.xml"),
        "{rooted:?}"
    );
}
