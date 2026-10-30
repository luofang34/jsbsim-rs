// SPDX-License-Identifier: LGPL-2.1-or-later
//! The Rust port running a script case, sampled exactly as
//! `tools/oracle/cpp/oracle_driver.cpp` samples C++: a row after `RunIC`,
//! then after every N-th `Run()`, N from `FGOutputType::SetRateHz`.

use std::path::Path;

use jsbsim::Fdm;
use jsbsim::{FileResolver, LoadOptions};

use crate::manifest::ScriptSpec;
use crate::series::TIME_COLUMN;
use crate::{OracleError, Series};

/// Why a Rust run stopped early.
#[derive(Debug)]
pub enum Stop {
    /// The port returned an error.
    Port(jsbsim::Error),
    /// The case cannot be run as specified.
    Oracle(OracleError),
}

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Port(e) => write!(f, "Rust port: {e}"),
            Self::Oracle(e) => e.fmt(f),
        }
    }
}

fn port_error(e: jsbsim::Error) -> Stop {
    Stop::Port(e)
}

/// Runs `spec` with files from `root` (a JSBSim tree or the vendored
/// subset in `oracle/inputs`). An error part-way returns the rows sampled
/// so far together with the error.
pub fn run_script(root: &Path, spec: &ScriptSpec) -> (Series, Option<Stop>) {
    run_script_with(root, spec, LoadOptions::default())
}

/// [`run_script`] with the port loading with `options`.
pub fn run_script_with(
    root: &Path,
    spec: &ScriptSpec,
    options: LoadOptions,
) -> (Series, Option<Stop>) {
    let mut series = Series {
        columns: std::iter::once(TIME_COLUMN.to_owned())
            .chain(spec.properties.iter().cloned())
            .collect(),
        rows: Vec::new(),
    };
    let error = sample(root, spec, options, &mut series).err();
    (series, error)
}

fn sample(
    root: &Path,
    spec: &ScriptSpec,
    options: LoadOptions,
    series: &mut Series,
) -> Result<(), Stop> {
    let resolver = FileResolver::new(root);
    let mut fdm = Fdm::with_options(options).map_err(port_error)?;
    if let Some(name) = &spec.ground_callback {
        crate::ground::install(&mut fdm, name).map_err(Stop::Oracle)?;
    }
    if let Some(path) = &spec.aircraft_path {
        fdm.set_aircraft_path(path);
    }
    fdm.load_script(&resolver, &spec.script)
        .map_err(port_error)?;
    let dt = fdm.models.exec.dt;
    let every = (0.5 + 1.0 / (dt * spec.rate_hz)) as u32;
    if every < 1 {
        return Err(Stop::Oracle(OracleError::Provenance(format!(
            "rate {} Hz is faster than dt {dt}",
            spec.rate_hz
        ))));
    }
    let end_time = spec.end_time.unwrap_or(f64::INFINITY);
    fdm.run_ic().map_err(port_error)?;
    // As the driver (and JSBSim's command-line program): an initial
    // condition's trim request runs once, and its result is not checked.
    let mode = fdm.trim_requested();
    if mode != jsbsim::trim::TrimMode::None {
        jsbsim::trim::Trim::new(&fdm, mode)
            .do_trim(&mut fdm)
            .map_err(port_error)?;
    }
    row(&mut fdm, spec, series)?;
    let mut frames = 0u32;
    let mut result = fdm.run().map_err(port_error)?;
    frames = frames.wrapping_add(1);
    if frames.is_multiple_of(every) {
        row(&mut fdm, spec, series)?;
    }
    while result && fdm.models.exec.sim_time <= end_time {
        result = fdm.run().map_err(port_error)?;
        frames = frames.wrapping_add(1);
        if frames.is_multiple_of(every) {
            row(&mut fdm, spec, series)?;
        }
    }
    Ok(())
}

fn row(fdm: &mut Fdm, spec: &ScriptSpec, series: &mut Series) -> Result<(), Stop> {
    let mut values = vec![fdm.models.exec.sim_time];
    for p in &spec.properties {
        values.push(fdm.get_property(p).map_err(port_error)?);
    }
    series.rows.push(values);
    Ok(())
}
