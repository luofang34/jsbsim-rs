// SPDX-License-Identifier: LGPL-2.1-or-later
//! FCS probe; twin of `tools/oracle/cpp/probe_fcs.cpp`. A load that needs a
//! construct the port does not cover yet prints `unsupported <construct>
//! (<where>)` instead of `ok`. The wall clock the core is given is the
//! one a `clock` line pins, else the real time, as the C++ probe's.

use std::path::PathBuf;

use jsbsim::Fdm;
use jsbsim::FileResolver;
use jsbsim::oracle_support::c_numeric::format_g;

use super::ProbeContext;

#[derive(Default)]
struct Probe {
    root: PathBuf,
    aircraft: String,
    systems: String,
    watched: Vec<String>,
    clock: Option<i64>,
    fdm: Option<Fdm>,
}

/// Seconds since the Unix epoch now.
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

impl Probe {
    fn print(&mut self) -> Result<String, jsbsim::Error> {
        let mut out = String::new();
        let Some(fdm) = self.fdm.as_mut() else {
            return Ok(" no-exec".to_owned());
        };
        for p in &self.watched {
            let value = match fdm.node(p)? {
                Some(node) => format_g(fdm.read_node(node)?, 17),
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
        let number = |i: usize| -> f64 {
            args.get(i)
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        match op {
            "dt" => {
                fdm.set_dt(number(0));
                Ok(" ok".to_owned())
            }
            "set" => {
                fdm.set_property(args.first().copied().unwrap_or_default(), number(1))?;
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
            _ => Ok(" unknown".to_owned()),
        }
    }

    fn run(&mut self, op: &str, args: &[&str], context: &ProbeContext) -> String {
        match op {
            "clock" => {
                let seconds = args
                    .first()
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(0);
                self.clock = Some(seconds);
                if let Some(fdm) = self.fdm.as_mut() {
                    fdm.models.exec.wall_clock = seconds;
                }
                " ok".to_owned()
            }
            "paths" => {
                let arg = |i: usize| args.get(i).copied().unwrap_or_default().to_owned();
                self.root = context.input_dir.join(arg(0));
                self.aircraft = arg(1);
                self.systems = arg(2);
                " ok".to_owned()
            }
            "new" => match Fdm::new() {
                Ok(mut fdm) => {
                    fdm.models.exec.wall_clock = self.clock.unwrap_or_else(now);
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
