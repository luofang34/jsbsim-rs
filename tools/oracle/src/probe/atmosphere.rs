// SPDX-License-Identifier: LGPL-2.1-or-later
//! Atmosphere probe; twin of `tools/oracle/cpp/probe_atmosphere.cpp`.

use jsbsim::c_numeric::format_g;
use jsbsim::fdm::Fdm;
use jsbsim::models::atmosphere::{PressureUnit, TemperatureUnit};

const PROPERTIES: [&str; 21] = [
    "atmosphere/T-R",
    "atmosphere/rho-slugs_ft3",
    "atmosphere/P-psf",
    "atmosphere/a-fps",
    "atmosphere/T-sl-R",
    "atmosphere/rho-sl-slugs_ft3",
    "atmosphere/a-sl-fps",
    "atmosphere/theta",
    "atmosphere/sigma",
    "atmosphere/delta",
    "atmosphere/a-ratio",
    "atmosphere/density-altitude",
    "atmosphere/pressure-altitude",
    "atmosphere/delta-T",
    "atmosphere/SL-graded-delta-T",
    "atmosphere/P-sl-psf",
    "atmosphere/dew-point-R",
    "atmosphere/vapor-pressure-psf",
    "atmosphere/saturated-vapor-pressure-psf",
    "atmosphere/RH",
    "atmosphere/vapor-fraction-ppm",
];

const TEMP_UNITS: [(&str, TemperatureUnit); 4] = [
    ("F", TemperatureUnit::Fahrenheit),
    ("C", TemperatureUnit::Celsius),
    ("R", TemperatureUnit::Rankine),
    ("K", TemperatureUnit::Kelvin),
];

const PRESS_UNITS: [(&str, PressureUnit); 4] = [
    ("psf", PressureUnit::Psf),
    ("mbar", PressureUnit::Millibars),
    ("pa", PressureUnit::Pascals),
    ("inhg", PressureUnit::InchesHg),
];

fn temp_unit(s: &str) -> TemperatureUnit {
    TEMP_UNITS
        .iter()
        .find(|(n, _)| *n == s)
        .map_or(TemperatureUnit::Rankine, |(_, u)| *u)
}

fn press_unit(s: &str) -> PressureUnit {
    PRESS_UNITS
        .iter()
        .find(|(n, _)| *n == s)
        .map_or(PressureUnit::Psf, |(_, u)| *u)
}

#[derive(Default)]
struct Out(String);

impl Out {
    fn num(&mut self, tag: &str, v: f64) {
        self.0.push(' ');
        self.0.push_str(tag);
        self.0.push(' ');
        self.0.push_str(&format_g(v, 17));
    }
}

fn properties(fdm: &mut Fdm) -> Result<String, jsbsim::Error> {
    let mut o = Out::default();
    for p in PROPERTIES {
        let v = fdm.get_property(p)?;
        o.num(p, v);
    }
    Ok(o.0)
}

fn at(fdm: &Fdm, h: f64) -> String {
    let a = &fdm.models.atmosphere;
    let mut o = Out::default();
    o.num("T", a.get_temperature(h));
    o.num("P", a.get_pressure(h));
    o.num("rho", a.get_density(h));
    o.num("a", a.get_sound_speed(h));
    o.num("theta", a.get_temperature_ratio_at(h));
    o.num("stdT", a.get_std_temperature(h));
    o.num("stdP", a.get_std_pressure(h));
    o.num("stdrho", a.get_std_density(h));
    o.num("stdtheta", a.get_std_temperature_ratio(h));
    o.0
}

fn units(fdm: &Fdm) -> String {
    let a = &fdm.models.atmosphere;
    let mut o = Out::default();
    for (n, u) in TEMP_UNITS {
        o.num(&format!("bias-{n}"), a.get_temperature_bias(u));
        o.num(
            &format!("gradient-{n}"),
            a.get_temperature_delta_gradient(u),
        );
        o.num(&format!("dew-{n}"), a.get_dew_point(u));
    }
    for (n, u) in PRESS_UNITS {
        o.num(&format!("psl-{n}"), a.get_pressure_sl(u));
        o.num(&format!("vapor-{n}"), a.get_vapor_pressure(u));
        o.num(&format!("saturated-{n}"), a.get_saturated_vapor_pressure(u));
    }
    o.num("mu", a.get_absolute_viscosity());
    o.num("nu", a.get_kinematic_viscosity());
    o.num("stdTsl", a.get_std_temperature_sl());
    o.num("Tsl", a.get_temperature_sl());
    o.0
}

/// Calls the setter `op` names; false when `op` is not one.
fn setter(fdm: &mut Fdm, op: &str, args: &[&str]) -> bool {
    let word = |i: usize| args.get(i).copied().unwrap_or_default();
    let num = |i: usize| word(i).parse::<f64>().unwrap_or_default();
    let a = &mut fdm.models.atmosphere;
    match op {
        "settemp" => a.set_temperature(num(0), num(1), temp_unit(word(2))),
        "settempsl" => a.set_temperature_sl(num(0), temp_unit(word(1))),
        "bias" => a.set_temperature_bias(temp_unit(word(0)), num(1)),
        "graded" => a.set_sl_temperature_graded_delta(temp_unit(word(0)), num(1)),
        "gradedh" => a.set_temperature_graded_delta(num(0), num(1), temp_unit(word(2))),
        "psl" => a.set_pressure_sl(press_unit(word(0)), num(1)),
        "resett" => a.reset_sl_temperature(),
        "resetp" => a.reset_sl_pressure(),
        "dew" => a.set_dew_point(temp_unit(word(0)), num(1)),
        "vapor" => a.set_vapor_pressure(press_unit(word(0)), num(1)),
        "rh" => a.set_relative_humidity(num(0)),
        "ppm" => a.set_vapor_mass_fraction_ppm(num(0)),
        _ => return false,
    }
    true
}

fn step(fdm: &mut Fdm, op: &str, args: &[&str]) -> Result<String, jsbsim::Error> {
    let num = |i: usize| {
        args.get(i)
            .and_then(|w| w.parse::<f64>().ok())
            .unwrap_or_default()
    };
    match op {
        "alt" | "hold" => {
            fdm.models.atmosphere.input.altitude_asl = num(0);
            fdm.run_atmosphere(op == "hold")?;
        }
        "init" => fdm.init_atmosphere()?,
        "set" => {
            let path = args.first().copied().unwrap_or_default();
            fdm.set_property(path, num(1))?;
        }
        "at" => return Ok(at(fdm, num(0))),
        "units" => return Ok(units(fdm)),
        _ if setter(fdm, op, args) => {}
        _ => return Ok(" unknown".to_owned()),
    }
    properties(fdm)
}

/// Executes the operations in `input`, one per line.
pub fn run(input: &str) -> Result<String, jsbsim::Error> {
    let mut fdm = Fdm::new()?;
    let mut out = String::new();
    for line in input.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let op = words.first().copied().unwrap_or_default();
        let result = step(&mut fdm, op, words.get(1..).unwrap_or_default())?;
        out.push_str(line);
        out.push_str(" =>");
        out.push_str(&result);
        out.push('\n');
    }
    Ok(out)
}
