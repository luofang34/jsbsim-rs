// SPDX-License-Identifier: LGPL-2.1-or-later
//! `oracle nasa control-law`: f16.xml's hand translation of F16_control.dml
//! and F16_gnc.dml checked frame by frame. Check-cases run in C++ JSBSim
//! and in the port, sampled every frame; each frame's inputs (state, pilot
//! and autopilot commands, trim inputs) are fed to the independent
//! DAVE-ML evaluator, whose surface, power and intermediate values must
//! equal those f16.xml's control system produced in that frame. Each
//! sampled row holds the state the control system ran on: in a frame
//! FGPropagate integrates first, then the control system runs on the new
//! state, and nothing after it changes the state.

use std::collections::BTreeMap;

use tracing::{error, info};

use super::daveml::{Maths, Model};
use super::{NasaManifest, package, rig, roots};
use crate::upstream::Paths;
use crate::{OracleError, Series};

/// The check-cases run, one per kind of manoeuvre and guidance.
pub const CASES: [&str; 4] = ["Atmos_13p1", "Atmos_13p3", "Atmos_15", "Atmos_16"];

/// The F-16 scripts' frame rate (dt = 0.0025 s), so that every frame is
/// sampled.
const FRAME_RATE_HZ: f64 = 400.0;

/// Largest difference allowed, relative to the value or 1 when smaller,
/// for the control law's variables: a few units in the last place, though
/// both runs reproduce the DAVE-ML bit for bit.
const TOLERANCE: f64 = 1e-15;
/// The same for f16.xml's air data against FGAuxiliary's, which computes
/// them by other formulas (it reaches 5e-14).
const AIR_DATA_TOLERANCE: f64 = 1e-13;

/// DAVE-ML state inputs and the JSBSim properties of the same frame. The
/// air data are f16.xml's own, computed from this frame's state, since
/// FGAuxiliary's run only after the control system; [`AIR_DATA`] checks
/// them against FGAuxiliary's at the end of the frame.
const STATE: [(&str, &str); 12] = [
    ("altMsl", "position/h-sl-ft"),
    ("Vequiv", "f16/air/ve-kts"),
    ("alpha", "f16/air/alpha-deg"),
    ("beta", "f16/air/beta-deg"),
    ("phi", "attitude/phi-deg"),
    ("theta", "attitude/theta-deg"),
    ("psi", "attitude/psi-deg"),
    ("pb", "velocities/p-rad_sec"),
    ("qb", "velocities/q-rad_sec"),
    ("rb", "velocities/r-rad_sec"),
    ("ownshipN_deg", "position/lat-geod-deg"),
    ("ownshipE_deg", "position/long-gc-deg"),
];

/// Commands and trim inputs, as f16.xml reads them.
const COMMANDS: [(&str, &str); 13] = [
    ("throttle", "f16/control/pilot-throttle"),
    ("longStk", "fcs/elevator-cmd-norm"),
    ("latStk", "fcs/aileron-cmd-norm"),
    ("pedal", "fcs/rudder-cmd-norm"),
    ("sasOn", "f16/control/sas-on"),
    ("apOn", "f16/control/ap-on"),
    ("keasCmd", "f16/control/keas-cmd"),
    ("altCmd", "f16/control/alt-cmd-ft"),
    ("circlePoleSW", "f16/gnc/circle-pole"),
    ("throttleTrim", "fcs/throttle-cmd-norm[0]"),
    ("longStkTrim", "fcs/pitch-trim-cmd-norm"),
    ("latOffset", "f16/gnc/lat-offset-ft"),
    ("baseChiCmd", "f16/gnc/base-course-cmd-deg"),
];

/// The course inputs F16_control.dml takes and F16_gnc.dml computes.
const COURSE: [&str; 2] = ["latOffset", "baseChiCmd"];

/// DAVE-ML variables compared, with the f16.xml properties holding them.
const OUTPUTS: [(&str, &str); 30] = [
    ("fsasOn", "f16/control/force-sas-on"),
    ("deltaThetaCmd", "f16/control/delta-pitch-cmd-deg"),
    ("thetaCmd", "f16/control/pitch-cmd-deg"),
    ("deltaChiCmd", "f16/control/course-correction-deg"),
    ("phiCmd", "f16/control/bank-cmd-deg"),
    ("deltaVequiv", "f16/control/delta-keas"),
    ("deltaAlpha", "f16/control/delta-alpha-deg"),
    ("deltaPhi", "f16/control/delta-phi-deg"),
    ("deltaTheta", "f16/control/delta-theta-deg"),
    ("longLQRsw", "f16/control/long-lqr"),
    ("throttleLQRsw", "f16/control/throttle-lqr"),
    ("latLQRsw", "f16/control/lat-lqr"),
    ("dirLQRsw", "f16/control/dir-lqr"),
    ("totLongStk", "f16/control/total-long-cmd"),
    ("totLatStk", "f16/control/total-lat-cmd"),
    ("totPedal", "f16/control/total-dir-cmd"),
    ("totThrottle", "f16/control/total-throttle"),
    ("el", "f16/control/el-deg"),
    ("ail", "f16/control/ail-deg"),
    ("rdr", "f16/control/rdr-deg"),
    ("PWR", "f16/control/pwr-pct"),
    ("latOffset", "f16/gnc/lat-offset-ft"),
    ("baseChiCmd", "f16/gnc/base-course-cmd-deg"),
    ("latOffsetPolar", "f16/gnc/lat-offset-polar-ft"),
    ("degEFromTgt", "f16/gnc/deg-e-from-tgt"),
    ("ownshipN_ft", "f16/gnc/ft-n-from-tgt"),
    ("ownshipE_ft", "f16/gnc/ft-e-from-tgt"),
    ("ftFromTgt", "f16/gnc/radius-from-tgt-ft"),
    (
        "baseChiCmdEquatorIDL",
        "f16/gnc/base-course-cmd-equator-idl-deg",
    ),
    ("latOffsetEquatorIDL", "f16/gnc/lat-offset-equator-idl-ft"),
];

const GUIDANCE: &str = "f16/gnc/guidance-on";

/// F16_gnc.dml's radiusFromTgt, the one power of the models.
const RADIUS: (&str, &str) = ("ftFromTgt", "f16/gnc/radius-from-tgt-ft");

/// f16.xml's air data, with FGAuxiliary's of the same frame.
const AIR_DATA: [(&str, &str, &str); 3] = [
    ("air alpha", "f16/air/alpha-deg", "aero/alpha-deg"),
    ("air beta", "f16/air/beta-deg", "aero/beta-deg"),
    ("air Vequiv", "f16/air/ve-kts", "velocities/ve-kts"),
];

/// Every property a run samples.
fn properties() -> Vec<&'static str> {
    let mut p: Vec<&str> = Vec::new();
    for name in STATE
        .iter()
        .chain(&COMMANDS)
        .chain(&OUTPUTS)
        .map(|x| x.1)
        .chain(AIR_DATA.iter().map(|a| a.2))
        .chain([GUIDANCE])
    {
        if !p.contains(&name) {
            p.push(name);
        }
    }
    p
}

/// Largest difference of one variable over a run.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Worst {
    /// |DAVE-ML - JSBSim|.
    pub diff: f64,
    /// Relative to the value, or to 1 when smaller.
    pub rel: f64,
    /// Time, s.
    pub time: f64,
}

/// The DAVE-ML inputs of `series`' row `row` for `model`: every state
/// and command the model takes or initialises, except the course inputs
/// under guidance, which F16_gnc.dml computes.
fn given(
    model: &Model,
    series: &Series,
    row: usize,
    guidance: bool,
) -> Result<BTreeMap<String, f64>, OracleError> {
    let mut given = BTreeMap::new();
    for (id, property) in STATE.iter().chain(&COMMANDS) {
        if model.is_settable(id) && !(guidance && COURSE.contains(id)) {
            given.insert((*id).to_owned(), rig::value(series, row, property)?);
        }
    }
    Ok(given)
}

fn note(worst: &mut BTreeMap<&'static str, Worst>, id: &'static str, a: f64, b: f64, time: f64) {
    let diff = (a - b).abs();
    let rel = diff / b.abs().max(1.0);
    let w = worst.entry(id).or_default();
    if rel > w.rel || rel.is_nan() {
        *w = Worst { diff, rel, time };
    }
}

/// Evaluates each row of `series` with F16_control.dml (`control`), or
/// with F16_gnc.dml (`gnc`) where the row has guidance on, and returns
/// the worst difference of each compared variable.
///
/// With `port_power`, F16_gnc.dml's one power, the circumnavigator's
/// radius (pow(x, 0.5)), takes the run's own value: the port's pow
/// (jsbsim::elementary) may differ from the platform's in the last digit,
/// which the lateral LQR would carry to 1e-10 of the aileron. That value
/// is checked instead against the platform's pow of the run's own
/// ownshipN_ft and ownshipE_ft, which are compared as usual.
pub fn compare_series(
    (control, gnc): (&Model, &Model),
    series: &Series,
    port_power: bool,
) -> Result<BTreeMap<&'static str, Worst>, OracleError> {
    let guided = series.columns.iter().position(|c| c == GUIDANCE);
    let mut worst: BTreeMap<&'static str, Worst> = BTreeMap::new();
    for (row, values) in series.rows.iter().enumerate() {
        let guidance = guided.and_then(|g| values.get(g)).is_some_and(|&g| g > 0.5);
        let model = if guidance { gnc } else { control };
        let mut inputs = given(model, series, row, guidance)?;
        let time = series.time(row).unwrap_or(f64::NAN);
        if port_power && model.has(RADIUS.0) {
            let radius = rig::value(series, row, RADIUS.1)?;
            let e = rig::value(series, row, "f16/gnc/ft-e-from-tgt")?;
            let n = rig::value(series, row, "f16/gnc/ft-n-from-tgt")?;
            note(
                &mut worst,
                "ftFromTgt (pow)",
                (e * e + n * n).powf(0.5),
                radius,
                time,
            );
            inputs.insert(RADIUS.0.to_owned(), radius);
        }
        let dml = model.evaluate(&inputs)?;
        for (id, property) in OUTPUTS {
            if (!guidance && COURSE.contains(&id)) || (port_power && id == RADIUS.0) {
                continue;
            }
            if let Some(&v) = dml.get(id) {
                note(&mut worst, id, v, rig::value(series, row, property)?, time);
            }
        }
        for (id, ours, auxiliary) in AIR_DATA {
            if series.columns.iter().any(|c| c == auxiliary) {
                let a = rig::value(series, row, ours)?;
                note(&mut worst, id, a, rig::value(series, row, auxiliary)?, time);
            }
        }
    }
    Ok(worst)
}

/// Logs `worst` of `label`; returns how many variables exceed the bound.
fn report(label: &str, worst: &BTreeMap<&'static str, Worst>) -> usize {
    let mut failures = 0usize;
    for (id, w) in worst {
        let line = format!(
            "nasa control-law {label} {id:<14} max |DAVE-ML - JSBSim| {:.3e} ({:.3e} relative) at t={:.4}",
            w.diff, w.rel, w.time
        );
        let bound = if id.starts_with("air ") {
            AIR_DATA_TOLERANCE
        } else {
            TOLERANCE
        };
        if w.rel <= bound {
            info!("{line}");
        } else {
            failures = failures.wrapping_add(1);
            error!("{line}: FAIL");
        }
    }
    failures
}

/// Runs [`CASES`] at every frame in C++ and Rust and compares the control
/// law; returns how many (case, run, variable) comparisons failed.
pub fn check_blocking(paths: &Paths) -> Result<usize, OracleError> {
    let models = package::unpack_blocking(paths)?;
    let control = Model::from_element(
        "F16_control.dml",
        &package::model_blocking(&models, "F16_control.dml")?,
    )?;
    let gnc = Model::from_element(
        "F16_gnc.dml",
        &package::model_blocking(&models, "F16_gnc.dml")?,
    )?;
    let manifest = NasaManifest::read_blocking(&paths.repo_root)?;
    let props = properties();
    let mut failures = 0usize;
    for name in CASES {
        let case = manifest
            .case
            .iter()
            .find(|c| c.name == name)
            .ok_or_else(|| OracleError::NasaCheck {
                check: "control-law".to_owned(),
                message: format!("{name} is not in oracle/nasa/manifest.toml"),
            })?;
        let (root, _) = roots(paths, case);
        let spec = rig::spec(&case.script, FRAME_RATE_HZ, None, &props);
        let run_dir = paths
            .work_dir()
            .join("run")
            .join(format!("nasa-control-law-{name}"));
        let runs = rig::run_both_blocking(paths, &root, &spec, &run_dir)?;
        let runs = [
            ("C++", &runs.cpp, Maths::Platform, false),
            ("Rust", &runs.rust, Maths::LibmCrate, true),
        ];
        for (who, series, maths, port_power) in runs {
            let control = control.clone().with_maths(maths);
            let gnc = gnc.clone().with_maths(maths);
            info!(
                "nasa control-law {name} {who}: {} frames, F16_gnc.dml where guidance is on, else F16_control.dml",
                series.rows.len()
            );
            let worst = compare_series((&control, &gnc), series, port_power)?;
            failures = failures.wrapping_add(report(&format!("{name} {who}"), &worst));
        }
    }
    Ok(failures)
}

#[cfg(test)]
mod tests;
