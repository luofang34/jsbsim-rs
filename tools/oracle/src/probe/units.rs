// SPDX-License-Identifier: LGPL-2.1-or-later
//! Units probe; twin of `tools/oracle/cpp/probe_units.cpp`.

use std::rc::Rc;

use jsbsim::oracle_support::c_numeric::format_g;
use jsbsim::oracle_support::units;
use jsbsim::oracle_support::xml::{Element, XmlError};

fn constant(name: &str) -> f64 {
    match name {
        "radtodeg" => units::RADTODEG,
        "degtorad" => units::DEGTORAD,
        "hptoftlbssec" => units::HPTOFTLBSSEC,
        "psftoinhg" => units::PSFTOINHG,
        "psftopa" => units::PSFTOPA,
        "fttom" => units::FTTOM,
        "ktstofps" => units::KTSTOFPS,
        "fpstokts" => units::FPSTOKTS,
        "inchtoft" => units::INCHTOFT,
        "m3toft3" => units::M3TOFT3,
        "in3tom3" => units::IN3TOM3,
        "inhgtopa" => units::INHGTOPA,
        "slugtolb" => units::SLUGTOLB,
        "lbtoslug" => units::LBTOSLUG,
        "kgtolb" => units::KGTOLB,
        _ => units::KGTOSLUG,
    }
}

fn temperature(function: &str, v: f64) -> f64 {
    match function {
        "KelvinToFahrenheit" => units::kelvin_to_fahrenheit(v),
        "CelsiusToRankine" => units::celsius_to_rankine(v),
        "RankineToCelsius" => units::rankine_to_celsius(v),
        "KelvinToRankine" => units::kelvin_to_rankine(v),
        "RankineToKelvin" => units::rankine_to_kelvin(v),
        "FahrenheitToCelsius" => units::fahrenheit_to_celsius(v),
        "CelsiusToFahrenheit" => units::celsius_to_fahrenheit(v),
        "CelsiusToKelvin" => units::celsius_to_kelvin(v),
        "KelvinToCelsius" => units::kelvin_to_celsius(v),
        _ => units::feet_to_meters(v),
    }
}

fn convert(from: &str, to: &str) -> String {
    let file: Rc<str> = Rc::from("");
    let mut parent = Element::new("p", file.clone(), 0);
    let mut v = Element::new("v", file, 0);
    v.set_attribute("unit", from);
    v.add_data("1");
    parent.add_child(v);
    match parent.find_element_value_as_number_convert_to("v", to) {
        Ok(value) => format_g(value, 17),
        Err(XmlError::UnknownUnit { .. }) => "unknown".to_owned(),
        Err(_) => "inconvertible".to_owned(),
    }
}

/// Executes the operations in `input`, one per line.
pub fn run(input: &str) -> String {
    let mut out = String::new();
    for line in input.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let arg = |i: usize| words.get(i).copied().unwrap_or_default();
        let result = match arg(0) {
            "convert" => convert(arg(1), arg(2)),
            "const" => format_g(constant(arg(1)), 17),
            "temp" => format_g(temperature(arg(1), arg(2).parse().unwrap_or_default()), 17),
            _ => "unknown op".to_owned(),
        };
        out.push_str(&format!("{line} => {result}\n"));
    }
    out
}
