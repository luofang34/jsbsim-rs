// SPDX-License-Identifier: LGPL-2.1-or-later
//! `oracle nasa limit-cycle`: guards the F-16 scripts' integration step.
//! Check-case 13.2 changes only the airspeed command, so once the speed
//! step's transient has passed the wings stay level; a step or integrator
//! too coarse for the lateral control law instead sets it into a roll
//! limit cycle of tens of deg/s (120 Hz with Adams-Bashforth 4 reaches
//! 31 deg/s), against about 0.0025 deg/s when converged.

use tracing::{error, info};

use super::{NasaManifest, roots};
use crate::upstream::Paths;
use crate::{OracleError, Series};

/// The case run.
const CASE: &str = "Atmos_13p2";
/// Roll rate sampled: the TM's bodyAngularRateWrtEi_deg_s_Roll.
const ROLL_RATE: &str = "velocities/pi-rad_sec";
/// After the speed step's transient.
const AFTER_S: f64 = 10.0;
/// Largest |roll rate| allowed after [`AFTER_S`], deg/s.
const MAX_DEG_S: f64 = 0.01;
/// Samples per second, as `oracle nasa compare` samples.
const RATE_HZ: f64 = 10.0;

/// Largest |`property`| in `series` after `after` s, scaled by `scale`,
/// with its time; `None` when nothing is sampled then.
#[must_use]
pub fn max_after(series: &Series, property: &str, after: f64, scale: f64) -> Option<(f64, f64)> {
    let column = series.columns.iter().position(|c| c == property)?;
    series
        .rows
        .iter()
        .filter_map(|r| Some((*r.first()?, *r.get(column)?)))
        .filter(|&(t, _)| t > after)
        .map(|(t, v)| ((v * scale).abs(), t))
        .reduce(|a, b| if b.0 > a.0 || b.0.is_nan() { b } else { a })
}

/// Logs the verdict of `who`'s run; returns whether it passed.
fn judge(who: &str, series: &Series) -> bool {
    match max_after(series, ROLL_RATE, AFTER_S, 180.0 / std::f64::consts::PI) {
        Some((p, t)) if p < MAX_DEG_S => {
            info!(
                "nasa limit-cycle {CASE} {who}: max |p| after t={AFTER_S} s {p:.3e} deg/s at t={t:.2}, below {MAX_DEG_S}"
            );
            true
        }
        Some((p, t)) => {
            error!(
                "nasa limit-cycle {CASE} {who}: max |p| after t={AFTER_S} s {p:.3e} deg/s at t={t:.2}, not below {MAX_DEG_S}: FAIL (roll limit cycle)"
            );
            false
        }
        None => {
            error!("nasa limit-cycle {CASE} {who}: nothing sampled after t={AFTER_S} s: FAIL");
            false
        }
    }
}

/// Runs [`CASE`] in C++ and Rust; returns how many runs show a limit cycle.
pub fn check_blocking(paths: &Paths) -> Result<usize, OracleError> {
    let manifest = NasaManifest::read_blocking(&paths.repo_root)?;
    let case = manifest
        .case
        .iter()
        .find(|c| c.name == CASE)
        .ok_or_else(|| OracleError::NasaCheck {
            check: "limit-cycle".to_owned(),
            message: format!("{CASE} is not in oracle/nasa/manifest.toml"),
        })?;
    let (root, _) = roots(paths, case);
    let spec = super::rig::spec(&case.script, RATE_HZ, None, &[ROLL_RATE]);
    let run_dir = paths.work_dir().join("run").join("nasa-limit-cycle");
    let runs = super::rig::run_both_blocking(paths, &root, &spec, &run_dir)?;
    let passed = [("C++", &runs.cpp), ("Rust", &runs.rust)]
        .into_iter()
        .filter(|(who, s)| judge(who, s))
        .count();
    Ok(2usize.saturating_sub(passed))
}

#[cfg(test)]
mod tests;
