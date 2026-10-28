// SPDX-License-Identifier: LGPL-2.1-or-later
//! Initial condition probe; twin of
//! `tools/oracle/cpp/probe_initial_condition.cpp`.
//!
//! A load the port refuses prints `unsupported <construct>`, which the case
//! manifest declares.

use std::path::Path;

use jsbsim::FileResolver;
use jsbsim::c_numeric::format_g;
use jsbsim::fdm::Fdm;
use jsbsim::initial_condition::SpeedSet;

use super::ProbeContext;

const PROPERTIES: [&str; 49] = [
    "vc-kts",
    "ve-kts",
    "vg-kts",
    "vt-kts",
    "mach",
    "roc-fpm",
    "gamma-deg",
    "alpha-deg",
    "beta-deg",
    "theta-deg",
    "phi-deg",
    "psi-true-deg",
    "lat-gc-deg",
    "long-gc-deg",
    "h-sl-ft",
    "h-agl-ft",
    "terrain-elevation-ft",
    "vg-fps",
    "vt-fps",
    "vw-bx-fps",
    "vw-by-fps",
    "vw-bz-fps",
    "vw-north-fps",
    "vw-east-fps",
    "vw-down-fps",
    "vw-mag-fps",
    "vw-dir-deg",
    "roc-fps",
    "u-fps",
    "v-fps",
    "w-fps",
    "vn-fps",
    "ve-fps",
    "vd-fps",
    "gamma-rad",
    "alpha-rad",
    "theta-rad",
    "beta-rad",
    "phi-rad",
    "psi-true-rad",
    "lat-gc-rad",
    "long-gc-rad",
    "p-rad_sec",
    "q-rad_sec",
    "r-rad_sec",
    "lat-geod-rad",
    "lat-geod-deg",
    "geod-alt-ft",
    "targetNlf",
];

/// `TrimMode::tNone`: the port refuses every other trim request.
const TRIM_NONE: i32 = 6;

fn number(v: f64) -> String {
    format_g(v, 17)
}

fn speed_set_index(s: SpeedSet) -> i32 {
    match s {
        SpeedSet::Vt => 0,
        SpeedSet::Vc => 1,
        SpeedSet::Ve => 2,
        SpeedSet::Mach => 3,
        SpeedSet::Uvw => 4,
        SpeedSet::Ned => 5,
        SpeedSet::Vg => 6,
    }
}

/// Below this horizontal wind speed (ft/s) the wind is round-off left by
/// the airspeed minus the ground velocity, and its direction is arbitrary.
const NOISE_WIND: f64 = 1e-9;

fn print(fdm: &mut Fdm) -> Result<String, jsbsim::Error> {
    let mut out = String::new();
    let noise_wind = fdm.models.ic.wind_mag_fps() < NOISE_WIND;
    for name in PROPERTIES {
        let v = fdm.get_property(&format!("ic/{name}"))?;
        let label = match name {
            "vw-dir-deg" if noise_wind => "vw-dir-deg-of-noise",
            _ => name,
        };
        out.push_str(&format!(" {label} {}", number(v)));
    }
    let ic = &fdm.models.ic;
    out.push_str(" ecef");
    for v in ic.position().ecef().to_array() {
        out.push_str(&format!(" {}", number(v)));
    }
    out.push_str(" q");
    for v in ic.orientation().to_array() {
        out.push_str(&format!(" {}", number(v)));
    }
    out.push_str(&format!(" epa {}", number(ic.earth_position_angle())));
    out.push_str(&format!(" speedset {}", speed_set_index(ic.speed_set())));
    out.push_str(&format!(" trim {TRIM_NONE}"));
    Ok(out)
}

fn printed(fdm: &mut Fdm) -> String {
    print(fdm).unwrap_or_else(|e| format!(" error {e}"))
}

#[derive(Default)]
struct Probe {
    fdm: Option<Fdm>,
}

fn load(fdm: &mut Fdm, rel: &str, input_dir: &Path) -> String {
    let resolver = FileResolver::new(input_dir);
    let (dir, name) = rel.rsplit_once('/').unwrap_or((".", rel));
    match fdm.load_initial_conditions(&resolver, dir, name) {
        Ok(result) => format!(" {result}{}", printed(fdm)),
        Err(jsbsim::Error::Unsupported { construct, .. }) => format!(" unsupported {construct}"),
        Err(e) => {
            tracing::debug!("load-error: {e}");
            " load-error".to_owned()
        }
    }
}

impl Probe {
    fn run(&mut self, words: &[&str], context: &ProbeContext) -> String {
        let arg = |i: usize| words.get(i).copied().unwrap_or_default();
        if matches!(arg(0), "new" | "load") {
            let mut fdm = match Fdm::new() {
                Ok(fdm) => fdm,
                Err(e) => return format!(" error {e}"),
            };
            let out = if arg(0) == "new" {
                printed(&mut fdm)
            } else {
                load(&mut fdm, arg(1), &context.input_dir)
            };
            self.fdm = Some(fdm);
            return out;
        }
        let Some(fdm) = self.fdm.as_mut() else {
            return " no-exec".to_owned();
        };
        match arg(0) {
            "reload" => load(fdm, arg(1), &context.input_dir),
            "set" => {
                if let Err(e) = fdm.set_property(arg(1), arg(2).parse().unwrap_or_default()) {
                    return format!(" error {e}");
                }
                printed(fdm)
            }
            _ => " unknown".to_owned(),
        }
    }
}

/// Executes the operations in `input`, one per line.
pub fn run(input: &str, context: &ProbeContext) -> String {
    let mut probe = Probe::default();
    let mut out = String::new();
    for line in input.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let result = probe.run(&words, context);
        out.push_str(&format!("{line} =>{result}\n"));
    }
    out
}
