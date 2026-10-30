// SPDX-License-Identifier: LGPL-2.1-or-later
//! The `check-sincos` diagnostic, run by scripts/oracle-sincos.sh.
//!
//! The committed references come from C++ as Clang compiles it, which on
//! Apple platforms fuses each `sin(x)`/`cos(x)` pair into
//! `__sincos_stret`, and the port uses the `libm` crate; their last-digit
//! differences are what the tolerance rows of these cases cover. Here C++
//! is rebuilt with [`SEPARATE_SINCOS_CXX_FLAGS`] in its own directory, its
//! references are written under that directory, and the port, linked with
//! the platform maths library in place of the `libm` crate, must
//! reproduce them bit for bit. The same holds for the check-cases of
//! `oracle/nasa/manifest.toml` named in [`NASA_CASES`], whose references
//! are generated the same way from the scripts `oracle nasa compare` runs.
//!
//! GeographicLib's `Math::sincosd` (Math.cpp) still calls `__sincos_stret`
//! under these flags, through libc++'s `std::sin` and `std::cos`. A case
//! whose output that changes belongs in [`EXCEPTIONS`], citing it; the
//! geodesic and waypoint cases are listed in [`CASES`] because it does not
//! change theirs.
//!
//! [`SEPARATE_SINCOS_CXX_FLAGS`]: crate::upstream::SEPARATE_SINCOS_CXX_FLAGS

use std::path::PathBuf;
use std::process::ExitCode;

use tracing::{error, info, warn};

use crate::cases::{self, Fresh, report_transcript};
use crate::manifest::{self, CaseSpec, INPUT_FILE, Manifest, REFERENCE_FILE, TRANSCRIPT_FILE};
use crate::nasa::NasaManifest;
use crate::transcript::compare_transcript_declaring;
use crate::upstream::{self, Paths};
use crate::{OracleError, Series, ToleranceTable};

mod nasa;
#[cfg(test)]
mod tests;

/// Golden cases that must be bit-identical: the piston, gear and trim
/// cases this diagnostic was made for, and every other golden, since all
/// of them are.
pub const CASES: &[&str] = &[
    "737_cruise",
    "737_cruise_steady_turn",
    "737_test",
    "B737_Runway",
    "B747_script1",
    "Concorde_rotate_test",
    "Concorde_runway_test",
    "F4N_runway_test",
    "J2460",
    "J2461",
    "L410",
    "L4102",
    "Short_S23_1",
    "Short_S23_2",
    "Short_S23_3",
    "Short_S23_4",
    "Submarine_Scout_1",
    "T37",
    "T38",
    "Test_F450_Launch",
    "ZLT-NT-moored-1",
    "accelerometer_on_ground",
    "ah1s_flight_test",
    "atmosphere",
    "auxiliary",
    "b171",
    "b737_runway_new",
    "ball",
    "ball-60s",
    "ball_chute",
    "ball_chute-60s",
    "ball_orbit",
    "ball_orbit-60s",
    "ball_orbit_accelerometers",
    "ball_orbit_g_torque",
    "ball_orbit_g_torque-60s",
    "ball_orbit_phase",
    "ball_orbit_phase-60s",
    "ball_spin_accelerometers",
    "c1721",
    "c1722",
    "c1722_inertia",
    "c1723",
    "c1724",
    "c172_cross_wind",
    "c172_cruise_8K",
    "c172_elevation_test",
    "c172_elevator_doublet",
    "c172_head_wind",
    "c172_runway_at_rest_cg_shift",
    "c3101",
    "c3104",
    "c3105",
    "cannonball",
    "cannonball-60s",
    "engine_indexed_props_post",
    "engine_indexed_props_table",
    "f16_runway_test",
    "f16_test",
    "fcs-absolute-paths",
    "fcs-accelerometer",
    "fcs-actuator-bug1503",
    "fcs-actuator-clipto",
    "fcs-actuator-forms",
    "fcs-actuator-hardover",
    "fcs-actuator-rate-linear",
    "fcs-actuator-rate-property",
    "fcs-angles",
    "fcs-angles-gnc",
    "fcs-base",
    "fcs-deadband",
    "fcs-distributor",
    "fcs-distributor-forms",
    "fcs-filter",
    "fcs-filter-forms",
    "fcs-function",
    "fcs-gain",
    "fcs-gain-types",
    "fcs-gyro",
    "fcs-integrators",
    "fcs-kinematic-forms",
    "fcs-kinematic-initial-value",
    "fcs-kinematic-noscale",
    "fcs-kinematic-timing",
    "fcs-linear-actuator-forms",
    "fcs-linear-actuator-gh485",
    "fcs-linear-actuator-rotations",
    "fcs-magnetometer",
    "fcs-magnetometer-upstream",
    "fcs-sensor",
    "fcs-sensor-noise",
    "fcs-summer-deadband",
    "fcs-switch",
    "fcs-switch-forms",
    "fcs-waypoint",
    "fcs-waypoint-errors",
    "fcs-waypoint-units",
    "function-errors",
    "function-ops",
    "function-random",
    "functions-basic",
    "gear_rover_deck_heaving",
    "gear_rover_deck_moving",
    "gear_rover_deck_rotating",
    "gear_rover_drop",
    "gear_rover_retract",
    "gear_rover_slope",
    "gear_rover_taxi",
    "geodesic",
    "gust-tests",
    "initial-condition",
    "lighter-than-air",
    "linearization",
    "mass-balance",
    "math",
    "mk82",
    "mk82_aero_limits",
    "mk82_atmosphere_override",
    "mk82_axial",
    "mk82_body",
    "mk82_stability",
    "mk82_wind",
    "nasa-atmos-02",
    "ov10_runway",
    "p51_start_test",
    "p51d_tail_wind",
    "paraglider_rotate_test",
    "piston_boost_boeing314",
    "piston_governor_c310",
    "piston_start_c310",
    "piston_takeoff",
    "planet",
    "properties",
    "propulsion-tanks",
    "random",
    "rotations",
    "rudder_kick",
    "sensor-random-seed",
    "sim_primer",
    "table-unit-tests",
    "tables",
    "trim",
    "trim_westward_737",
    "tripod_ground_centered",
    "tripod_ground_eccentric",
    "tripod_input",
    "tripod_mass_change",
    "turbine_spool_N1SpoolDown",
    "turbine_spool_N1SpoolUp",
    "turbine_spool_N2SpoolDown",
    "turbine_spool_N2SpoolUp",
    "turboprop_power_vc_function",
    "turbulence-tests",
    "units",
    "unusable_fuel",
    "weather-balloon",
    "wf-to-bf",
    "winds",
    "x151",
    "x152",
    "x153",
    "xml",
];

/// A golden case left out, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exception {
    /// Golden case name.
    pub case: &'static str,
    /// Why it cannot be bit-identical here.
    pub reason: &'static str,
}

/// Golden cases that cannot be bit-identical even with these flags.
pub const EXCEPTIONS: &[Exception] = &[];

/// Check-cases of `oracle/nasa/manifest.toml` that must be bit-identical.
/// Their aircraft is not upstream, so they have no golden; each is run as
/// `oracle nasa compare` runs it. The F-16 cases are here because their
/// `[case.parity]` bounds cover only the gate builds' sin and cos.
pub const NASA_CASES: &[&str] = &[
    "Atmos_11",
    "Atmos_12",
    "Atmos_13p1",
    "Atmos_13p2",
    "Atmos_13p3",
    "Atmos_13p4",
    "Atmos_15",
    "Atmos_16",
];

/// Runs `names`, or [`CASES`] and [`NASA_CASES`] when empty, and fails unless every one is
/// bit-identical. `paths.build` must be [`upstream::Build::SeparateSincos`].
pub fn check_blocking(paths: &Paths, names: &[String]) -> Result<ExitCode, OracleError> {
    if paths.build != upstream::Build::SeparateSincos {
        return Err(OracleError::Sincos(format!(
            "needs the separate-sincos build, not {:?}",
            paths.build
        )));
    }
    if !platform_libm_linked() {
        return Err(OracleError::Sincos(
            "the port is linked with the libm crate, not the platform maths library; \
             run scripts/oracle-sincos.sh"
                .to_owned(),
        ));
    }
    upstream::build_blocking(paths)?;
    let names: Vec<String> = if names.is_empty() {
        warn_unlisted_blocking(paths)?;
        CASES
            .iter()
            .chain(NASA_CASES)
            .map(|&c| c.to_owned())
            .collect()
    } else {
        names.to_vec()
    };
    let nasa_manifest = NasaManifest::read_blocking(&paths.repo_root)?;
    let mut identical = 0usize;
    for case in &names {
        let same = if NASA_CASES.contains(&case.as_str()) {
            nasa::check_case_blocking(paths, &nasa_manifest, case)?
        } else {
            check_case_blocking(paths, case)?
        };
        if same {
            identical = identical.wrapping_add(1);
        }
    }
    for exception in EXCEPTIONS {
        warn!("{}: not checked: {}", exception.case, exception.reason);
    }
    let summary = format!(
        "{identical} of {} cases bit-identical, {} excepted",
        names.len(),
        EXCEPTIONS.len()
    );
    if identical == names.len() {
        info!("check-sincos: {summary}");
        Ok(ExitCode::SUCCESS)
    } else {
        error!("check-sincos: {summary}");
        Ok(ExitCode::FAILURE)
    }
}

/// Logs goldens neither in [`CASES`] nor in [`EXCEPTIONS`], which the
/// diagnostic does not run.
fn warn_unlisted_blocking(paths: &Paths) -> Result<(), OracleError> {
    for case in manifest::list_cases_blocking(&paths.goldens_dir())? {
        let listed = CASES.contains(&case.as_str()) || EXCEPTIONS.iter().any(|e| e.case == case);
        if !listed {
            warn!("{case}: golden not listed in tools/oracle/src/sincos.rs; not checked");
        }
    }
    Ok(())
}

/// Whether the port calls the platform maths library, as it does under
/// scripts/oracle-sincos.sh: its patch makes the `libm` crate answer as
/// the platform does, and its `platform-libm` feature makes
/// `jsbsim::elementary` forward to it. The `libm` crate's `exp(1)` alone
/// is 1 ulp from the platform's.
// Reason: the check is of what the `libm` crate itself returns.
#[allow(clippy::disallowed_methods)]
fn platform_libm_linked() -> bool {
    jsbsim::elementary::FORWARDS_TO_LIBM
        && (0..64).all(|i| {
            let x = std::hint::black_box(f64::from(i) / 16.0);
            libm::exp(x).to_bits() == x.exp().to_bits()
                && libm::pow(x, 0.3).to_bits() == x.powf(0.3).to_bits()
                && libm::sin(x).to_bits() == x.sin().to_bits()
        })
}

fn reference_dir(paths: &Paths, case: &str) -> PathBuf {
    paths.work_dir().join("references").join(case)
}

/// Regenerates `case` with the separate-sincos C++ build, runs the port
/// on it and logs the outcome; returns whether they are bit-identical.
fn check_case_blocking(paths: &Paths, case: &str) -> Result<bool, OracleError> {
    let golden = manifest::case_dir(&paths.goldens_dir(), case);
    let manifest = Manifest::read_blocking(&golden)?;
    let (fresh, _) = cases::run_fresh_blocking(paths, case, &manifest)?;
    let dir = reference_dir(paths, case);
    std::fs::create_dir_all(&dir).map_err(|e| OracleError::io("creating", &dir, e))?;
    match (&manifest.case, fresh) {
        (CaseSpec::Script(spec), Fresh::Series(reference)) => {
            reference.write_blocking(&dir.join(REFERENCE_FILE))?;
            let rust = crate::port_script::run_script(&spec.rust_root(&paths.repo_root), spec);
            Ok(report_series(case, &reference, rust))
        }
        (CaseSpec::Transcript(spec), Fresh::Transcript(reference)) => {
            let path = dir.join(TRANSCRIPT_FILE);
            std::fs::write(&path, &reference).map_err(|e| OracleError::io("writing", &path, e))?;
            let context = crate::probe::ProbeContext {
                input_dir: golden.clone(),
                jsbsim_root: None,
            };
            let input_path = golden.join(INPUT_FILE);
            let input = std::fs::read_to_string(&input_path)
                .map_err(|e| OracleError::io("reading", &input_path, e))?;
            let candidate = crate::probe::run(&spec.probe, &input, &context)?;
            // No rows: every line must match byte for byte.
            let outcome = compare_transcript_declaring(
                case,
                &reference,
                &candidate,
                &ToleranceTable::default(),
                &spec.unsupported,
            )?;
            Ok(report_transcript(case, &outcome))
        }
        _ => Err(OracleError::Sincos(format!(
            "{case}: the C++ run produced the wrong kind of output"
        ))),
    }
}

/// Logs whether the Rust run `rust` reproduced `reference` bit for bit;
/// returns whether it did.
fn report_series(
    case: &str,
    reference: &Series,
    (candidate, stop): (Series, Option<crate::port_script::Stop>),
) -> bool {
    if let Some(stop) = stop {
        error!("{case}: the Rust run stopped: {stop}");
        return false;
    }
    match first_difference(reference, &candidate) {
        None => {
            info!(
                "{case}: bit-identical, {} rows x {} columns",
                reference.rows.len(),
                reference.columns.len()
            );
            true
        }
        Some(difference) => {
            error!("{case}: FIRST DIFFERENCE {difference}");
            false
        }
    }
}

/// The first value of `candidate` whose bits differ from `reference`'s
/// (any two NaNs agree), or a difference in shape.
#[must_use]
pub fn first_difference(reference: &Series, candidate: &Series) -> Option<String> {
    if reference.columns != candidate.columns {
        return Some(format!(
            "columns: reference {:?}, candidate {:?}",
            reference.columns, candidate.columns
        ));
    }
    for (ref_row, cand_row) in reference.rows.iter().zip(&candidate.rows) {
        let time = ref_row.first().copied().unwrap_or(f64::NAN);
        for ((property, &r), &c) in reference.columns.iter().zip(ref_row).zip(cand_row) {
            if r.to_bits() != c.to_bits() && !(r.is_nan() && c.is_nan()) {
                return Some(format!(
                    "t={time} {property}: reference={r:?} candidate={c:?}"
                ));
            }
        }
    }
    (reference.rows.len() != candidate.rows.len()).then(|| {
        format!(
            "reference has {} rows, candidate has {}",
            reference.rows.len(),
            candidate.rows.len()
        )
    })
}
