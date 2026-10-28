// SPDX-License-Identifier: LGPL-2.1-or-later
//! The check-case CSVs: one per simulator, each with its own subset of the
//! TM's standard output variables, matched to JSBSim properties by name.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::OracleError;

/// Degrees per radian.
const DEG_PER_RAD: f64 = 180.0 / std::f64::consts::PI;
/// Knots per ft/s, exactly as the TM's Table 73 defines it.
const KTS_PER_FPS: f64 = 1097.28 / 1852.0;

/// A TM output variable and the JSBSim property that yields it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Column {
    /// Variable name in the NASA CSV header.
    pub nasa: &'static str,
    /// JSBSim property sampled for it.
    pub property: &'static str,
    /// Factor from the property's unit to the variable's.
    pub scale: f64,
    /// Wrap-around period of an angle, in the variable's unit.
    pub period: Option<f64>,
}

const fn column(nasa: &'static str, property: &'static str) -> Column {
    Column {
        nasa,
        property,
        scale: 1.0,
        period: None,
    }
}

const fn scaled(nasa: &'static str, property: &'static str, scale: f64) -> Column {
    Column {
        nasa,
        property,
        scale,
        period: None,
    }
}

const fn angle(nasa: &'static str, property: &'static str) -> Column {
    Column {
        nasa,
        property,
        scale: 1.0,
        period: Some(360.0),
    }
}

/// Every TM variable the atmospheric check-cases record that JSBSim has.
pub const COLUMNS: &[Column] = &[
    column("gePosition_ft_X", "position/ecef-x-ft"),
    column("gePosition_ft_Y", "position/ecef-y-ft"),
    column("gePosition_ft_Z", "position/ecef-z-ft"),
    column("eiPosition_ft_X", "position/eci-x-ft"),
    column("eiPosition_ft_Y", "position/eci-y-ft"),
    column("eiPosition_ft_Z", "position/eci-z-ft"),
    column("feVelocity_ft_s_X", "velocities/v-north-fps"),
    column("feVelocity_ft_s_Y", "velocities/v-east-fps"),
    column("feVelocity_ft_s_Z", "velocities/v-down-fps"),
    column("eiVelocity_ft_s_X", "velocities/eci-x-fps"),
    column("eiVelocity_ft_s_Y", "velocities/eci-y-fps"),
    column("eiVelocity_ft_s_Z", "velocities/eci-z-fps"),
    column("altitudeMsl_ft", "position/h-sl-ft"),
    angle("longitude_deg", "position/long-gc-deg"),
    column("latitude_deg", "position/lat-geod-deg"),
    column("localGravity_ft_s2", "accelerations/gravity-ft_sec2"),
    angle("eulerAngle_deg_Yaw", "attitude/psi-deg"),
    column("eulerAngle_deg_Pitch", "attitude/theta-deg"),
    angle("eulerAngle_deg_Roll", "attitude/phi-deg"),
    scaled(
        "bodyAngularRateWrtEi_deg_s_Roll",
        "velocities/pi-rad_sec",
        DEG_PER_RAD,
    ),
    scaled(
        "bodyAngularRateWrtEi_deg_s_Pitch",
        "velocities/qi-rad_sec",
        DEG_PER_RAD,
    ),
    scaled(
        "bodyAngularRateWrtEi_deg_s_Yaw",
        "velocities/ri-rad_sec",
        DEG_PER_RAD,
    ),
    scaled("altitudeRateWrtMsl_ft_min", "velocities/h-dot-fps", 60.0),
    column("speedOfSound_ft_s", "atmosphere/a-fps"),
    column("airDensity_slug_ft3", "atmosphere/rho-slugs_ft3"),
    column("ambientPressure_lbf_ft2", "atmosphere/P-psf"),
    column("ambientTemperature_dgR", "atmosphere/T-R"),
    column("aero_bodyForce_lbf_X", "forces/fbx-aero-lbs"),
    column("aero_bodyForce_lbf_Y", "forces/fby-aero-lbs"),
    column("aero_bodyForce_lbf_Z", "forces/fbz-aero-lbs"),
    column("aero_bodyMoment_ftlbf_L", "moments/l-aero-lbsft"),
    column("aero_bodyMoment_ftlbf_M", "moments/m-aero-lbsft"),
    column("aero_bodyMoment_ftlbf_N", "moments/n-aero-lbsft"),
    column("mach", "velocities/mach"),
    column("dynamicPressure_lbf_ft2", "aero/qbar-psf"),
    scaled("trueAirspeed_nmi_h", "velocities/vt-fps", KTS_PER_FPS),
];

/// One simulator's recording: values by variable, keyed by time in
/// milliseconds so that the 100 Hz and 10 Hz recordings line up.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Simulator {
    /// File stem, e.g. `Atmos_02_sim_05`.
    pub name: String,
    /// Variable name to (time in ms to value). Empty cells are absent.
    pub values: BTreeMap<String, BTreeMap<i64, f64>>,
}

/// Time key of `t` seconds.
#[must_use]
pub fn time_key(t: f64) -> i64 {
    (t * 1000.0).round() as i64
}

impl Simulator {
    /// Parses a NASA CSV. Only the first of repeated header names counts.
    pub fn parse(name: &str, text: &str) -> Result<Self, OracleError> {
        let err = |line: usize, message: String| OracleError::Csv {
            source_name: name.to_owned(),
            line,
            message,
        };
        let mut lines = text.lines().enumerate();
        let header: Vec<&str> = lines
            .next()
            .map(|(_, l)| l.split(',').map(str::trim).collect())
            .ok_or_else(|| err(1, "empty file".to_owned()))?;
        if header.first() != Some(&"time") {
            return Err(err(1, "first column must be time".to_owned()));
        }
        let first: Vec<bool> = header
            .iter()
            .enumerate()
            .map(|(i, h)| header.iter().position(|x| x == h) == Some(i))
            .collect();
        let mut values: BTreeMap<String, BTreeMap<i64, f64>> = BTreeMap::new();
        for (index, line) in lines {
            if line.trim().is_empty() {
                continue;
            }
            let cells: Vec<&str> = line.split(',').map(str::trim).collect();
            let number = |cell: &str| {
                cell.parse::<f64>()
                    .map_err(|e| err(index + 1, format!("{cell:?}: {e}")))
            };
            let Some(time) = cells.first() else {
                continue;
            };
            let key = time_key(number(time)?);
            for ((col, cell), first) in header.iter().zip(&cells).zip(&first).skip(1) {
                if *first && !cell.is_empty() {
                    let value = number(cell)?;
                    values
                        .entry((*col).to_owned())
                        .or_default()
                        .insert(key, value);
                }
            }
        }
        Ok(Self {
            name: name.to_owned(),
            values,
        })
    }

    /// Value of `variable` at time key `key`.
    #[must_use]
    pub fn value(&self, variable: &str, key: i64) -> Option<f64> {
        self.values.get(variable)?.get(&key).copied()
    }
}

/// Every `*.csv` under `dir`, skipping macOS resource forks, sorted.
pub fn csv_files_blocking(dir: &Path) -> Result<Vec<PathBuf>, OracleError> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(d) = pending.pop() {
        let entries = std::fs::read_dir(&d).map_err(|e| OracleError::io("listing", &d, e))?;
        for entry in entries {
            let path = entry.map_err(|e| OracleError::io("listing", &d, e))?.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "__MACOSX" || name.starts_with("._") {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "csv") {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Reads every simulator recording under `dir`.
pub fn read_simulators_blocking(dir: &Path) -> Result<Vec<Simulator>, OracleError> {
    csv_files_blocking(dir)?
        .iter()
        .map(|path| {
            let text =
                std::fs::read_to_string(path).map_err(|e| OracleError::io("reading", path, e))?;
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            Simulator::parse(name, &text)
        })
        .collect()
}
