// SPDX-License-Identifier: LGPL-2.1-or-later
//! `oracle nasa mass`: the mass properties f16.xml gives JSBSim, in C++ and
//! in the port, against F16_inertia.dml with the center of mass at 25% MAC
//! (TM Table 7). JSBSim's inertia tensor carries the negated products of
//! inertia, so `inertia/ixz-slugs_ft2` must be -Ixz = -982 slug ft2 for the
//! TM's Ixz = +982.

use std::collections::BTreeMap;

use tracing::{error, info};

use super::daveml::Model;
use super::{package, rig};
use crate::upstream::Paths;
use crate::{OracleError, Series};

/// Center of mass of the check-cases, percent MAC (TM Table 7).
const CG_PCT_MAC: f64 = 25.0;
/// JSBSim's pounds per slug (FGJSBBase::slugtolb), which turns the mass
/// into the empty weight.
const LB_PER_SLUG: f64 = 32.174049;
/// Allowed difference, relative to the value or 1 when smaller: a few
/// units in the last place, for the rounding of the slug to pound and inch
/// to foot conversions.
const TOLERANCE: f64 = 1e-15;

const IC: &str = r#"<?xml version="1.0"?>
<initialize name="mass" version="2.0">
  <position frame="ECEF">
    <latitude unit="DEG" type="geodetic"> 36.0 </latitude>
    <longitude unit="DEG"> -75.0 </longitude>
    <altitudeMSL unit="FT"> 10000.0 </altitudeMSL>
  </position>
  <orientation unit="DEG" frame="LOCAL">
    <roll> 0.0 </roll> <pitch> 0.0 </pitch> <yaw> 0.0 </yaw>
  </orientation>
  <velocity unit="FT/SEC" frame="BODY">
    <x> 500.0 </x> <y> 0.0 </y> <z> 0.0 </z>
  </velocity>
</initialize>
"#;

const SCRIPT: &str = r#"<?xml version="1.0"?>
<runscript name="mass">
  <use aircraft="f16" initialize="mass"/>
  <run start="0.0" end="0.0" dt="0.0025"/>
</runscript>
"#;

const PROPERTIES: [&str; 13] = [
    "inertia/ixx-slugs_ft2",
    "inertia/iyy-slugs_ft2",
    "inertia/izz-slugs_ft2",
    "inertia/ixy-slugs_ft2",
    "inertia/ixz-slugs_ft2",
    "inertia/iyz-slugs_ft2",
    "inertia/mass-slugs",
    "inertia/empty-weight-lbs",
    "inertia/cg-x-in",
    "inertia/cg-y-in",
    "inertia/cg-z-in",
    "metrics/aero-rp-x-in",
    "metrics/aero-rp-z-in",
];

/// (what, JSBSim's value from `s`, F16_inertia.dml's value from `d`).
type Check = (
    &'static str,
    fn(&Value) -> f64,
    fn(&BTreeMap<String, f64>) -> f64,
);

/// The sampled properties of one run.
struct Value(BTreeMap<&'static str, f64>);

impl Value {
    fn get(&self, p: &str) -> f64 {
        self.0.get(p).copied().unwrap_or(f64::NAN)
    }
}

const CHECKS: [Check; 11] = [
    ("Ixx", |s| s.get("inertia/ixx-slugs_ft2"), |d| dv(d, "XIXX")),
    ("Iyy", |s| s.get("inertia/iyy-slugs_ft2"), |d| dv(d, "XIYY")),
    ("Izz", |s| s.get("inertia/izz-slugs_ft2"), |d| dv(d, "XIZZ")),
    (
        "tensor xy = -Ixy",
        |s| s.get("inertia/ixy-slugs_ft2"),
        |d| -dv(d, "XIXY"),
    ),
    (
        "tensor xz = -Ixz",
        |s| s.get("inertia/ixz-slugs_ft2"),
        |d| -dv(d, "XIZX"),
    ),
    (
        "tensor yz = -Iyz",
        |s| s.get("inertia/iyz-slugs_ft2"),
        |d| -dv(d, "XIYZ"),
    ),
    (
        "mass, slug",
        |s| s.get("inertia/mass-slugs"),
        |d| dv(d, "XMASS"),
    ),
    (
        "empty weight, lbf",
        |s| s.get("inertia/empty-weight-lbs"),
        |d| dv(d, "XMASS") * LB_PER_SLUG,
    ),
    (
        "CM forward of the MRC (AERORP), ft",
        |s| (s.get("metrics/aero-rp-x-in") - s.get("inertia/cg-x-in")) / 12.0,
        |d| dv(d, "DXCG"),
    ),
    (
        "CM right of the plane of symmetry, ft",
        |s| s.get("inertia/cg-y-in") / 12.0,
        |d| dv(d, "DYCG"),
    ),
    (
        "CM below the MRC (AERORP), ft",
        |s| (s.get("metrics/aero-rp-z-in") - s.get("inertia/cg-z-in")) / 12.0,
        |d| dv(d, "DZCG"),
    ),
];

/// A DAVE-ML value, NaN (which fails every comparison) when absent.
fn dv(d: &BTreeMap<String, f64>, id: &str) -> f64 {
    d.get(id).copied().unwrap_or(f64::NAN)
}

fn sampled(series: &Series) -> Result<Value, OracleError> {
    let mut v = BTreeMap::new();
    for p in PROPERTIES {
        v.insert(p, rig::value(series, 0, p)?);
    }
    Ok(Value(v))
}

/// F16_inertia.dml's outputs at [`CG_PCT_MAC`].
fn inertia_blocking(paths: &Paths) -> Result<BTreeMap<String, f64>, OracleError> {
    let models = package::unpack_blocking(paths)?;
    let dml = package::model_blocking(&models, "F16_inertia.dml")?;
    Model::from_element("F16_inertia.dml", &dml)?
        .evaluate(&BTreeMap::from([("CG_PCT_MAC".to_owned(), CG_PCT_MAC)]))
}

/// Logs each comparison of `label`'s run; returns how many failed.
fn report(label: &str, run: &Value, dml: &BTreeMap<String, f64>) -> usize {
    let mut failures = 0usize;
    for (what, jsbsim, expected) in CHECKS {
        let (got, want) = (jsbsim(run), expected(dml));
        let ok = (got - want).abs() <= TOLERANCE * want.abs().max(1.0);
        let line = format!("nasa mass {label}: {what}: JSBSim {got}, F16_inertia.dml {want}");
        if ok {
            info!("{line}: ok");
        } else {
            failures = failures.wrapping_add(1);
            error!("{line}: FAIL");
        }
    }
    failures
}

/// Loads f16.xml in C++ and Rust and compares its mass properties with
/// F16_inertia.dml; returns how many comparisons failed.
pub fn check_blocking(paths: &Paths) -> Result<usize, OracleError> {
    let dml = inertia_blocking(paths)?;
    let root = rig::create_blocking(paths, "mass", &rig::f16_text_blocking(paths)?)?;
    rig::add_file_blocking(&root, "aircraft/f16/mass.xml", IC)?;
    rig::add_file_blocking(&root, "scripts/mass.xml", SCRIPT)?;
    let spec = rig::spec("scripts/mass.xml", 400.0, Some(0.0), &PROPERTIES);
    let run_dir = root.with_file_name("cpp");
    let runs = rig::run_both_blocking(paths, &root, &spec, &run_dir)?;
    let cpp = report("C++", &sampled(&runs.cpp)?, &dml);
    let rust = report("Rust", &sampled(&runs.rust)?, &dml);
    Ok(cpp.wrapping_add(rust))
}

#[cfg(test)]
mod tests;
