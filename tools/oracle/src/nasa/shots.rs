// SPDX-License-Identifier: LGPL-2.1-or-later
//! `oracle nasa shots`: the checkData static shots of F16_aero.dml and
//! F16_prop.dml run through f16.xml's tables and functions, in C++ JSBSim
//! and in the port. The control system is replaced by fixed surface
//! deflections and power lever angle; each shot is one initial condition
//! read at t = 0. Every checked output, and every internal value the shot
//! lists that f16.xml exposes as `f16/aero/<varID>` or as a `f16/prop/`
//! thrust, must match within the shot's tolerance (or 1e-6 of the value,
//! or of 1 when smaller, where it gives none).

use std::fmt::Write as _;

use tracing::{error, info};

use super::daveml::{Shot, shots};
use super::{package, rig};
use crate::upstream::Paths;
use crate::{OracleError, Series};

/// Allowed difference where the checkData gives no tolerance.
const DEFAULT_TOLERANCE: f64 = 1e-6;
/// Wing area, span and chord of f16.xml (ft2, ft, ft).
const SREF: f64 = 300.0;
const BSPAN: f64 = 30.0;
const CBAR: f64 = 11.32;

/// The control inputs the shots set in place of F16_gnc's outputs.
const SURFACES: [(&str, &str); 3] = [
    ("elevatorDeflection", "f16/control/el-deg"),
    ("aileronDeflection", "f16/control/ail-deg"),
    ("rudderDeflection", "f16/control/rdr-deg"),
];
const PWR: &str = "f16/control/pwr-pct";

/// F16_prop.dml's internal values and outputs, with f16.xml's properties.
const PROP: [(&str, &str); 10] = [
    ("T_IDLE", "f16/prop/t-idle-lbf"),
    ("T_MIL", "f16/prop/t-mil-lbf"),
    ("T_MAX", "f16/prop/t-max-lbf"),
    ("FEX", "f16/prop/fex-lbf"),
    ("thrustBodyForce_X", "forces/fbx-external-lbs"),
    ("thrustBodyForce_Y", "forces/fby-external-lbs"),
    ("thrustBodyForce_Z", "forces/fbz-external-lbs"),
    ("thrustBodyMoment_Roll", "moments/l-external-lbsft"),
    ("thrustBodyMoment_Pitch", "moments/m-external-lbsft"),
    ("thrustBodyMoment_Yaw", "moments/n-external-lbsft"),
];

/// F16_aero.dml's outputs: (name, f16.xml property, reference length).
const AERO: [(&str, &str, f64); 6] = [
    ("aeroBodyForceCoefficient_X", "aero/force/f16-x", 1.0),
    ("aeroBodyForceCoefficient_Y", "aero/force/f16-y", 1.0),
    ("aeroBodyForceCoefficient_Z", "aero/force/f16-z", 1.0),
    (
        "aeroBodyMomentCoefficient_Roll",
        "aero/moment/f16-roll",
        BSPAN,
    ),
    (
        "aeroBodyMomentCoefficient_Pitch",
        "aero/moment/f16-pitch",
        CBAR,
    ),
    (
        "aeroBodyMomentCoefficient_Yaw",
        "aero/moment/f16-yaw",
        BSPAN,
    ),
];
const REFERENCES: [(&str, &str); 3] = [
    ("referenceWingChord", "metrics/cbarw-ft"),
    ("referenceWingSpan", "metrics/bw-ft"),
    ("referenceWingArea", "metrics/Sw-sqft"),
];

/// f16.xml with the control law and its air data replaced by fixed
/// control inputs, which each shot's script overrides.
fn without_control_law(f16: &str) -> Result<String, OracleError> {
    let mut text = f16.to_owned();
    for name in ["air_now", "F16_gnc"] {
        let open = format!("<system name=\"{name}\">");
        let start = text.find(&open).ok_or_else(|| missing(&open))?;
        let end = text
            .get(start..)
            .and_then(|rest| rest.find("</system>"))
            .map(|e| start + e + "</system>".len())
            .ok_or_else(|| missing("</system>"))?;
        text.replace_range(start..end, "");
    }
    let mut fixed = String::from("<system name=\"shot inputs\">\n");
    for (_, p) in SURFACES {
        writeln!(fixed, "    <property value=\"0\">{p}</property>").ok();
    }
    writeln!(
        fixed,
        "    <property value=\"0\">{PWR}</property>\n  </system>"
    )
    .ok();
    let at = text
        .find("<system name=\"F16_prop\">")
        .ok_or_else(|| missing("F16_prop"))?;
    text.insert_str(at, &fixed);
    Ok(text)
}

fn missing(what: &str) -> OracleError {
    OracleError::NasaCheck {
        check: "shots".to_owned(),
        message: format!("f16.xml has no {what}"),
    }
}

fn input(shot: &Shot, name: &str) -> Result<f64, OracleError> {
    shot.inputs
        .get(name)
        .copied()
        .ok_or_else(|| OracleError::NasaCheck {
            check: shot.name.clone(),
            message: format!("no checkInput {name}"),
        })
}

fn script(name: &str, overrides: &[(&str, f64)]) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\"?>\n<runscript name=\"{name}\">\n  \
         <use aircraft=\"f16\" initialize=\"{name}\"/>\n  \
         <run start=\"0.0\" end=\"0.0\" dt=\"0.0025\">\n"
    );
    for (p, v) in overrides {
        writeln!(s, "    <property value=\"{v:?}\"> {p} </property>").ok();
    }
    s.push_str("  </run>\n</runscript>\n");
    s
}

/// The aero shot's state: body velocity from airspeed, alpha and beta,
/// pitched by alpha so that the flight path is level, and body rates.
fn aero_ic(name: &str, shot: &Shot) -> Result<String, OracleError> {
    let vt = input(shot, "trueAirspeed")?;
    let alpha = input(shot, "angleOfAttack")?;
    let (a, b) = (
        alpha.to_radians(),
        input(shot, "angleOfSideslip")?.to_radians(),
    );
    let (u, v, w) = (vt * a.cos() * b.cos(), vt * b.sin(), vt * a.sin() * b.cos());
    let p = input(shot, "bodyAngularRate_Roll")?;
    let q = input(shot, "bodyAngularRate_Pitch")?;
    let r = input(shot, "bodyAngularRate_Yaw")?;
    Ok(format!(
        "<?xml version=\"1.0\"?>\n<initialize name=\"{name}\" version=\"2.0\">\n  \
         <position frame=\"ECEF\">\n    <latitude unit=\"DEG\" type=\"geodetic\"> 36.0 </latitude>\n    \
         <longitude unit=\"DEG\"> -75.0 </longitude>\n    <altitudeMSL unit=\"FT\"> 10013.0 </altitudeMSL>\n  \
         </position>\n  <orientation unit=\"DEG\" frame=\"LOCAL\">\n    <roll> 0 </roll>\n    \
         <pitch> {alpha:?} </pitch>\n    <yaw> 0 </yaw>\n  </orientation>\n  \
         <velocity unit=\"FT/SEC\" frame=\"BODY\">\n    <x> {u:?} </x>\n    <y> {v:?} </y>\n    \
         <z> {w:?} </z>\n  </velocity>\n  <attitude_rate unit=\"RAD/SEC\" frame=\"BODY\">\n    \
         <roll> {p:?} </roll>\n    <pitch> {q:?} </pitch>\n    <yaw> {r:?} </yaw>\n  \
         </attitude_rate>\n</initialize>\n"
    ))
}

fn prop_ic(name: &str, shot: &Shot) -> Result<String, OracleError> {
    let alt = input(shot, "altitudeMSL")?;
    let mach = input(shot, "mach")?;
    Ok(format!(
        "<?xml version=\"1.0\"?>\n<initialize name=\"{name}\">\n  \
         <latitude unit=\"DEG\"> 36.0 </latitude>\n  <longitude unit=\"DEG\"> -75.0 </longitude>\n  \
         <altitudeMSL unit=\"FT\"> {alt:?} </altitudeMSL>\n  <mach> {mach:?} </mach>\n  \
         <phi unit=\"DEG\"> 0 </phi>\n  <theta unit=\"DEG\"> 0 </theta>\n  <psi unit=\"DEG\"> 0 </psi>\n\
         </initialize>\n"
    ))
}

/// One comparison: (label, JSBSim's value, the model's, tolerance).
type Comparison = (String, f64, f64, f64);

fn tolerance(tol: f64, expected: f64) -> f64 {
    if tol > 0.0 {
        tol
    } else {
        DEFAULT_TOLERANCE * expected.abs().max(1.0)
    }
}

/// The comparisons of an aero shot whose run sampled `properties`.
fn aero_comparisons(
    shot: &Shot,
    series: &Series,
    internal: &[String],
) -> Result<Vec<Comparison>, OracleError> {
    let qs = rig::value(series, 0, "aero/qbar-psf")? * SREF;
    let mut out = Vec::new();
    for (name, property, length) in AERO {
        if let Some(&(want, tol)) = shot.outputs.get(name) {
            let got = rig::value(series, 0, property)? / (qs * length);
            out.push((name.to_owned(), got, want, tolerance(tol, want)));
        }
    }
    for (name, property) in REFERENCES {
        if let Some(&(want, tol)) = shot.outputs.get(name) {
            let got = rig::value(series, 0, property)?;
            out.push((name.to_owned(), got, want, tolerance(tol, want)));
        }
    }
    for id in internal {
        if let Some(&want) = shot.internal.get(id) {
            let got = rig::value(series, 0, &format!("f16/aero/{id}"))?;
            out.push((format!("internal {id}"), got, want, tolerance(0.0, want)));
        }
    }
    Ok(out)
}

fn prop_comparisons(shot: &Shot, series: &Series) -> Result<Vec<Comparison>, OracleError> {
    let mut out = Vec::new();
    for (name, property) in PROP {
        let expected = shot
            .outputs
            .get(name)
            .copied()
            .or_else(|| shot.internal.get(name).map(|&v| (v, 0.0)));
        if let Some((want, tol)) = expected {
            let got = rig::value(series, 0, property)?;
            out.push((name.to_owned(), got, want, tolerance(tol, want)));
        }
    }
    Ok(out)
}

/// Logs `comparisons` of `label`; returns how many failed and how many ran.
fn report(label: &str, comparisons: &[Comparison]) -> (usize, usize) {
    let mut failures = 0usize;
    for (what, got, want, tol) in comparisons {
        let d = got - want;
        let line =
            format!("{label} {what}: JSBSim {got}, DAVE-ML {want}, diff {d:.2e}, tol {tol:e}");
        if d.abs() <= *tol {
            info!("{line}");
        } else {
            failures = failures.wrapping_add(1);
            error!("{line}: FAIL");
        }
    }
    (failures, comparisons.len())
}

/// Totals of a shots run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    /// Shots run, in C++ and Rust each.
    pub shots: usize,
    /// Comparisons made.
    pub compared: usize,
    /// Comparisons outside their tolerance.
    pub failed: usize,
}

impl Tally {
    fn add(&mut self, (failed, compared): (usize, usize)) {
        self.failed = self.failed.wrapping_add(failed);
        self.compared = self.compared.wrapping_add(compared);
    }
}

/// Runs one family of shots (`kind` "aero" or "prop") and adds to `tally`.
fn run_family_blocking(
    paths: &Paths,
    root: &std::path::Path,
    kind: &str,
    shots: &[Shot],
    internal: &[String],
    tally: &mut Tally,
) -> Result<(), OracleError> {
    let mut properties: Vec<String> = ["aero/qbar-psf", "velocities/mach", "position/h-sl-ft"]
        .into_iter()
        .chain(AERO.iter().map(|a| a.1))
        .chain(REFERENCES.iter().map(|r| r.1))
        .chain(PROP.iter().map(|p| p.1))
        .map(str::to_owned)
        .collect();
    properties.extend(internal.iter().map(|id| format!("f16/aero/{id}")));
    let refs: Vec<&str> = properties.iter().map(String::as_str).collect();
    for (i, shot) in shots.iter().enumerate() {
        let name = format!("{kind}{i}");
        let (ic, overrides) = if kind == "aero" {
            let mut o = Vec::new();
            for (signal, p) in SURFACES {
                o.push((p, input(shot, signal)?));
            }
            (aero_ic(&name, shot)?, o)
        } else {
            (
                prop_ic(&name, shot)?,
                vec![(PWR, input(shot, "powerLeverAngle")?)],
            )
        };
        rig::add_file_blocking(root, &format!("aircraft/f16/{name}.xml"), &ic)?;
        let script_path = format!("scripts/{name}.xml");
        rig::add_file_blocking(root, &script_path, &script(&name, &overrides))?;
        let spec = rig::spec(&script_path, 400.0, Some(0.0), &refs);
        let run_dir = root.with_file_name(&name);
        let runs = rig::run_both_blocking(paths, root, &spec, &run_dir)?;
        for (who, series) in [("C++", &runs.cpp), ("Rust", &runs.rust)] {
            let label = format!("nasa shots {kind} {i} ({}) {who}", shot.name);
            let comparisons = if kind == "aero" {
                aero_comparisons(shot, series, internal)?
            } else {
                prop_comparisons(shot, series)?
            };
            tally.add(report(&label, &comparisons));
        }
        tally.shots = tally.shots.wrapping_add(1);
    }
    Ok(())
}

/// Runs every static shot of F16_aero.dml and F16_prop.dml in C++ and
/// Rust and logs each comparison.
pub fn check_blocking(paths: &Paths) -> Result<Tally, OracleError> {
    let models = package::unpack_blocking(paths)?;
    let f16 = without_control_law(&rig::f16_text_blocking(paths)?)?;
    let root = rig::create_blocking(paths, "shots", &f16)?;
    let mut tally = Tally::default();
    for (kind, file) in [("aero", "F16_aero.dml"), ("prop", "F16_prop.dml")] {
        let dml = package::model_blocking(&models, file)?;
        let family = shots(file, &dml)?;
        let mut internal: Vec<String> = family
            .iter()
            .flat_map(|s| s.internal.keys())
            .filter(|id| f16.contains(&format!("\"f16/aero/{id}\"")))
            .cloned()
            .collect();
        internal.sort();
        internal.dedup();
        if kind == "prop" {
            internal.clear();
        }
        let compared = if kind == "prop" {
            PROP.iter()
                .take(4)
                .map(|p| p.1)
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            internal
                .iter()
                .map(|id| format!("f16/aero/{id}"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        info!(
            "nasa shots {kind}: {} shots of {file}, internal values compared: {compared}",
            family.len()
        );
        run_family_blocking(paths, &root, kind, &family, &internal, &mut tally)?;
    }
    Ok(tally)
}

#[cfg(test)]
mod tests;
