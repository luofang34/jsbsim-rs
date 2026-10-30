// SPDX-License-Identifier: LGPL-2.1-or-later
//! Propulsion probe; twin of `tools/oracle/cpp/probe_propulsion.cpp`. A
//! load that needs a construct the port does not cover yet prints
//! `unsupported <construct> (<where>)` instead of `ok`.

use std::path::PathBuf;

use jsbsim::Fdm;
use jsbsim::FileResolver;
use jsbsim::math::Axis;
use jsbsim::oracle_support::c_numeric::format_g;
use jsbsim::oracle_support::models::propulsion::tank::{GrainType, TankType};

use super::ProbeContext;

fn number(v: f64) -> String {
    format_g(v, 17)
}

fn tanks(fdm: &mut Fdm) -> String {
    let m = &mut fdm.models;
    let p = &mut m.propulsion;
    let mut out = String::new();
    for (i, t) in p.tanks.iter().enumerate() {
        let tank_type = match t.tank_type {
            TankType::Unknown => 0,
            TankType::Fuel => 1,
            TankType::Oxidizer => 2,
        };
        let grain = match t.grain.kind {
            GrainType::Unknown => 0,
            GrainType::Cylindrical => 1,
            GrainType::EndBurning => 2,
        };
        out.push_str(&format!(" tank{i} {tank_type} {grain}"));
        let xyz = t.xyz();
        for v in [
            t.contents,
            t.pct_full,
            t.capacity,
            t.unusable(),
            t.density,
            xyz[Axis::X],
            xyz[Axis::Y],
            xyz[Axis::Z],
            t.standpipe,
            f64::from(t.priority),
            f64::from(u8::from(t.selected)),
            t.temperature,
            t.ixx,
            t.iyy,
            t.izz,
            t.external_flow,
        ] {
            out.push_str(&format!(" {}", number(v)));
        }
    }
    out.push_str(&format!(" weight {} moment", number(p.tanks_weight())));
    let moment = p.tanks_moment();
    for a in Axis::ALL {
        out.push_str(&format!(" {}", number(moment[a])));
    }
    out.push_str(" J");
    let j = p.calculate_tank_inertias(&m.mass_balance);
    for r in Axis::ALL {
        for c in Axis::ALL {
            out.push_str(&format!(" {}", number(j[(r, c)])));
        }
    }
    out
}

#[derive(Default)]
struct Probe {
    root: PathBuf,
    aircraft: String,
    systems: String,
    watched: Vec<String>,
    fdm: Option<Fdm>,
}

impl Probe {
    fn print(&mut self) -> Result<String, jsbsim::Error> {
        let mut out = String::new();
        let Some(fdm) = self.fdm.as_mut() else {
            return Ok(" no-exec".to_owned());
        };
        for p in &self.watched {
            let value = match fdm.node(p)? {
                Some(node) => number(fdm.read_node(node)?),
                None => "absent".to_owned(),
            };
            out.push_str(&format!(" {p} {value}"));
        }
        Ok(out)
    }

    fn load(&mut self, model: &str) -> String {
        let resolver = FileResolver::new(&self.root);
        let Some(fdm) = self.fdm.as_mut() else {
            return " no-exec".to_owned();
        };
        match fdm.load_model(&resolver, &self.aircraft, model) {
            Ok(()) => " ok".to_owned(),
            Err(jsbsim::Error::Unsupported { construct, path }) => {
                format!(" unsupported {construct} ({path})")
            }
            Err(e) => {
                tracing::debug!("{model}: load-error: {e}");
                " load-error".to_owned()
            }
        }
    }

    fn step(&mut self, op: &str, args: &[&str]) -> Result<String, jsbsim::Error> {
        let Some(fdm) = self.fdm.as_mut() else {
            return Ok(" no-exec".to_owned());
        };
        let number_at = |i: usize| -> f64 {
            args.get(i)
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        let tank_at = |i: usize| -> Option<usize> {
            args.get(i)
                .and_then(|s| s.parse::<i64>().ok())
                .and_then(|n| usize::try_from(n).ok())
        };
        match op {
            "dt" => {
                fdm.set_dt(number_at(0));
                Ok(" ok".to_owned())
            }
            "set" => {
                fdm.set_property(args.first().copied().unwrap_or_default(), number_at(1))?;
                Ok(" ok".to_owned())
            }
            "runic" => {
                fdm.run_ic()?;
                self.print()
            }
            "run" => {
                fdm.run()?;
                self.print()
            }
            "print" => self.print(),
            "tanks" => Ok(tanks(fdm)),
            "refuel" => {
                fdm.models.propulsion.do_refuel(number_at(0));
                Ok(tanks(fdm))
            }
            "dump" => {
                fdm.models.propulsion.dump_fuel(number_at(0));
                Ok(tanks(fdm))
            }
            "transfer" => {
                let p = &mut fdm.models.propulsion;
                let overage = p.transfer(tank_at(0), tank_at(1), number_at(2));
                Ok(format!(" {}", number(overage)))
            }
            "init" => {
                fdm.models.propulsion.init_model();
                Ok(tanks(fdm))
            }
            _ => Ok(" unknown".to_owned()),
        }
    }

    fn run(&mut self, op: &str, args: &[&str], context: &ProbeContext) -> String {
        match op {
            "paths" => {
                let arg = |i: usize| args.get(i).copied().unwrap_or_default().to_owned();
                self.root = context.input_dir.join(arg(0));
                self.aircraft = arg(1);
                self.systems = arg(2);
                " ok".to_owned()
            }
            "new" => match Fdm::new() {
                Ok(mut fdm) => {
                    fdm.set_aircraft_path(&self.aircraft);
                    fdm.set_systems_path(&self.systems);
                    self.fdm = Some(fdm);
                    " ok".to_owned()
                }
                Err(e) => {
                    tracing::debug!("new: {e}");
                    " error".to_owned()
                }
            },
            "watch" => {
                self.watched = args.iter().map(|s| (*s).to_owned()).collect();
                " ok".to_owned()
            }
            "load" => self.load(args.first().copied().unwrap_or_default()),
            _ => self.step(op, args).unwrap_or_else(|e| {
                tracing::debug!("{op}: error: {e}");
                " error".to_owned()
            }),
        }
    }
}

/// Runs the probe on `input`.
#[must_use]
pub fn run(input: &str, context: &ProbeContext) -> String {
    let mut probe = Probe::default();
    let mut out = String::new();
    for line in input.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let (op, args) = words
            .split_first()
            .map_or(("", &[][..]), |(op, a)| (*op, a));
        let result = probe.run(op, args, context);
        out.push_str(&format!("{line} =>{result}\n"));
    }
    out
}
