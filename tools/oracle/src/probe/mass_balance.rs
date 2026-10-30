// SPDX-License-Identifier: LGPL-2.1-or-later
//! Mass balance probe; twin of `tools/oracle/cpp/probe_mass_balance.cpp`.

use jsbsim::Fdm;
use jsbsim::math::Axis;
use jsbsim::oracle_support::FdmInternals as _;
use jsbsim::oracle_support::c_numeric::format_g;
use jsbsim::oracle_support::xml::parse_document;

use super::ProbeContext;

const PROPERTIES: [&str; 12] = [
    "mass-slugs",
    "weight-lbs",
    "empty-weight-lbs",
    "cg-x-in",
    "cg-y-in",
    "cg-z-in",
    "ixx-slugs_ft2",
    "iyy-slugs_ft2",
    "izz-slugs_ft2",
    "ixy-slugs_ft2",
    "ixz-slugs_ft2",
    "iyz-slugs_ft2",
];

const POINT_MASS: [&str; 4] = [
    "pointmass-weight-lbs",
    "pointmass-location-X-inches",
    "pointmass-location-Y-inches",
    "pointmass-location-Z-inches",
];

fn number(v: f64) -> String {
    format_g(v, 17)
}

fn print(fdm: &mut Fdm) -> Result<String, jsbsim::Error> {
    let mut out = String::new();
    for name in PROPERTIES {
        let v = fdm.get_property(&format!("inertia/{name}"))?;
        out.push_str(&format!(" {name}={}", number(v)));
    }
    for i in 0.. {
        let index = format!("[{i}]");
        if fdm
            .node(&format!("inertia/{}{index}", POINT_MASS[0]))?
            .is_none()
        {
            break;
        }
        for name in POINT_MASS {
            let v = fdm.get_property(&format!("inertia/{name}{index}"))?;
            out.push_str(&format!(" {name}{index}={}", number(v)));
        }
    }
    let mb = &fdm.models.mass_balance;
    out.push_str(" dcg");
    let dcg = mb.delta_xyz_cg();
    for i in Axis::ALL {
        out.push_str(&format!(" {}", number(dcg[i])));
    }
    for (label, m) in [(" J", mb.j()), (" Jinv", mb.jinv())] {
        out.push_str(label);
        for r in Axis::ALL {
            for c in Axis::ALL {
                out.push_str(&format!(" {}", number(m[(r, c)])));
            }
        }
    }
    Ok(out)
}

fn run_and_print(fdm: &mut Fdm) -> String {
    let m = &mut fdm.models;
    m.mass_balance
        .run(false, m.exec.hold_down, &mut m.propagate);
    print(fdm).unwrap_or_else(|e| format!(" error {e}"))
}

#[derive(Default)]
struct Probe {
    fdm: Option<Fdm>,
}

impl Probe {
    fn load(&mut self, kind: &str, rel: &str, context: &ProbeContext) -> String {
        self.fdm = None;
        let path = match kind {
            "file" => context.jsbsim_root.clone().unwrap_or_default().join(rel),
            _ => context.input_dir.join(rel),
        };
        let mut fdm = match Fdm::new() {
            Ok(fdm) => fdm,
            Err(e) => return format!(" error {e}"),
        };
        let Ok(bytes) = std::fs::read(&path) else {
            self.fdm = Some(fdm);
            return " missing".to_owned();
        };
        let loaded = parse_document(&bytes, rel)
            .map_err(jsbsim::Error::from)
            .and_then(|doc| {
                let el = if doc.name() == "mass_balance" {
                    Some(&doc)
                } else {
                    doc.find_element("mass_balance")
                };
                el.map(|el| fdm.load_mass_balance(el)).transpose()
            });
        let result = match loaded {
            Ok(Some(())) => run_and_print(&mut fdm),
            Ok(None) => " no-mass-balance".to_owned(),
            Err(jsbsim::Error::Unsupported { construct, .. }) => {
                format!(" unsupported {construct}")
            }
            Err(e) => {
                tracing::debug!("load-error: {e}");
                " load-error".to_owned()
            }
        };
        self.fdm = Some(fdm);
        result
    }

    fn run(&mut self, words: &[&str], context: &ProbeContext) -> String {
        let arg = |i: usize| words.get(i).copied().unwrap_or_default();
        if arg(0) == "load" {
            return self.load(arg(1), arg(2), context);
        }
        let Some(fdm) = self.fdm.as_mut() else {
            return " no-exec".to_owned();
        };
        match arg(0) {
            "set" => {
                fdm.set_property(arg(1), arg(2).parse().unwrap_or_default())
                    .ok();
                " ok".to_owned()
            }
            "run" => run_and_print(fdm),
            "init" => {
                fdm.models.mass_balance.init_model();
                " ok".to_owned()
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
