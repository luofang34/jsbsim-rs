// SPDX-License-Identifier: LGPL-2.1-or-later
//! Winds probe; twin of `tools/oracle/cpp/probe_winds.cpp`.

use jsbsim::c_numeric::format_g;
use jsbsim::fdm::Fdm;
use jsbsim::math::{ColumnVector3, Matrix33};

const PROPERTIES: [&str; 24] = [
    "atmosphere/psiw-rad",
    "atmosphere/wind-north-fps",
    "atmosphere/wind-east-fps",
    "atmosphere/wind-down-fps",
    "atmosphere/wind-mag-fps",
    "atmosphere/gust-north-fps",
    "atmosphere/gust-east-fps",
    "atmosphere/gust-down-fps",
    "atmosphere/turb-north-fps",
    "atmosphere/turb-east-fps",
    "atmosphere/turb-down-fps",
    "atmosphere/p-turb-rad_sec",
    "atmosphere/q-turb-rad_sec",
    "atmosphere/r-turb-rad_sec",
    "atmosphere/turb-type",
    "atmosphere/turb-rate",
    "atmosphere/turb-gain",
    "atmosphere/turb-rhythmicity",
    "atmosphere/turbulence/milspec/windspeed_at_20ft_AGL-fps",
    "atmosphere/turbulence/milspec/severity",
    "atmosphere/total-wind-north-fps",
    "atmosphere/total-wind-east-fps",
    "atmosphere/total-wind-down-fps",
    "atmosphere/randomseed",
];

fn num(out: &mut String, tag: &str, v: f64) {
    out.push_str(&format!(" {tag} {}", format_g(v, 17)));
}

fn print(fdm: &mut Fdm) -> String {
    let mut out = String::new();
    for p in PROPERTIES {
        match fdm.get_property(p) {
            Ok(v) => num(&mut out, p, v),
            Err(e) => return format!(" error {e}"),
        }
    }
    let w = &fdm.models.winds;
    let (magnitude, direction) = (w.turbulence_magnitude(), w.turbulence.direction);
    num(&mut out, "turb-magnitude", magnitude);
    num(&mut out, "turb-direction", direction);
    out
}

/// Numbers after the operation word, missing ones as zero like `>>`
/// into an initialised variable.
fn numbers<const N: usize>(words: &[&str]) -> [f64; N] {
    let mut v = [0.0; N];
    for (slot, word) in v.iter_mut().zip(words.iter().skip(1)) {
        *slot = word.parse().unwrap_or_default();
    }
    v
}

fn run_op(fdm: &mut Fdm, words: &[&str]) -> String {
    let op = words.first().copied().unwrap_or_default();
    let input = &mut fdm.models.winds.input;
    match op {
        "set" => {
            let name = words.get(1).copied().unwrap_or_default();
            let [_, v] = numbers::<2>(words);
            if let Err(e) = fdm.set_property(name, v) {
                return format!(" error {e}");
            }
            print(fdm)
        }
        "uvw" => {
            let [u, v, w] = numbers(words);
            input.uvw = ColumnVector3::new(u, v, w);
            " ok".to_owned()
        }
        "tl2b" | "tw2b" => {
            let [a, b, c, d, e, f, g, h, i] = numbers(words);
            let m = Matrix33::new(a, b, c, d, e, f, g, h, i);
            if op == "tl2b" {
                input.tl2b = m;
            } else {
                input.tw2b = m;
            }
            " ok".to_owned()
        }
        "dt" => {
            [input.total_delta_t] = numbers(words);
            " ok".to_owned()
        }
        "alt" => {
            [input.altitude_asl, input.distance_agl, input.wingspan] = numbers(words);
            " ok".to_owned()
        }
        "rate" => {
            let rate = words.get(1).and_then(|w| w.parse().ok()).unwrap_or(1);
            fdm.models.winds.base.rate = rate;
            " ok".to_owned()
        }
        "windned" => {
            let [n, e, d] = numbers(words);
            fdm.models.winds.set_wind_ned(ColumnVector3::new(n, e, d));
            print(fdm)
        }
        "run" | "hold" => match fdm.models.winds.run(op == "hold") {
            Ok(()) => print(fdm),
            Err(jsbsim::Error::Unsupported { construct, .. }) => {
                format!(" unsupported {construct}")
            }
            Err(e) => format!(" error {e}"),
        },
        "init" => {
            fdm.models.winds.init_model();
            print(fdm)
        }
        "print" => print(fdm),
        _ => " unknown".to_owned(),
    }
}

/// Executes the operations in `input`, one per line.
pub fn run(input: &str) -> String {
    let mut fdm = match Fdm::new() {
        Ok(fdm) => fdm,
        Err(e) => return format!("error {e}\n"),
    };
    let mut out = String::new();
    for line in input.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let result = run_op(&mut fdm, &words);
        out.push_str(&format!("{line} =>{result}\n"));
    }
    out
}
