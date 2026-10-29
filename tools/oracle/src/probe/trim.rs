// SPDX-License-Identifier: LGPL-2.1-or-later
//! Trim probe; twin of `tools/oracle/cpp/probe_trim.cpp`, which describes
//! the input and the transcript.

use std::path::PathBuf;

use jsbsim::FileResolver;
use jsbsim::c_numeric::format_g;
use jsbsim::fdm::Fdm;
use jsbsim::linearization::Linearization;
use jsbsim::trim::{Control, Trim, TrimMode, TrimReport};

use super::ProbeContext;

fn number(v: f64) -> String {
    format_g(v, 17)
}

/// `Control`'s value in JSBSim's enum.
fn control_index(c: Control) -> i32 {
    match c {
        Control::Throttle => 0,
        Control::Beta => 1,
        Control::Alpha => 2,
        Control::Elevator => 3,
        Control::Aileron => 4,
        Control::Rudder => 5,
        Control::AltAgl => 6,
        Control::Theta => 7,
        Control::Phi => 8,
        Control::Gamma => 9,
        Control::PitchTrim => 10,
        Control::RollTrim => 11,
        Control::YawTrim => 12,
        Control::Heading => 13,
    }
}

/// What `Recorder::Record` prints.
fn record(time: f64, r: &TrimReport) -> String {
    let mut out = format!(
        " trim t {} mode {} result {} its {}",
        number(time),
        r.mode as i32,
        i32::from(r.success),
        r.total_iterations
    );
    for a in &r.axes {
        out.push_str(&format!(" axis {} {} control {} state {} tolerance {} sub {} successful {} stability {} runs {}",
            a.state.name(),
            control_index(a.control),
            number(a.control_value),
            number(a.state_value),
            number(a.tolerance),
            number(a.sub_iterations),
            number(a.successful),
            a.stability,
            a.run_count));
    }
    out
}

#[derive(Default)]
struct Probe {
    input_dir: PathBuf,
    root: String,
    aircraft: String,
    systems: String,
    model: String,
    watched: Vec<String>,
    fdm: Option<Fdm>,
    trims_seen: u32,
    trims: String,
}

impl Probe {
    fn resolver(&self) -> FileResolver {
        FileResolver::new(self.input_dir.join(&self.root))
    }

    fn print(fdm: &mut Fdm, watched: &[String]) -> Result<String, jsbsim::Error> {
        let mut out = String::new();
        for p in watched {
            let value = match fdm.node(p)? {
                Some(node) => number(fdm.read_node(node)?),
                None => "absent".to_owned(),
            };
            out.push_str(&format!(" {p} {value}"));
        }
        Ok(out)
    }

    /// The trim the executive ran since the last call, if any, with the
    /// watched properties now.
    fn collect(&mut self) -> Result<(), jsbsim::Error> {
        let Some(fdm) = self.fdm.as_mut() else {
            return Ok(());
        };
        if fdm.trims_run() == self.trims_seen {
            return Ok(());
        }
        self.trims_seen = fdm.trims_run();
        if let Some(r) = fdm.last_trim() {
            self.trims
                .push_str(&record(fdm.models.exec.sim_time, &r.clone()));
        }
        let after = Self::print(fdm, &self.watched)?;
        self.trims.push_str(&format!(" after{after}"));
        Ok(())
    }

    fn step(&mut self) -> Result<bool, jsbsim::Error> {
        let Some(fdm) = self.fdm.as_mut() else {
            return Ok(false);
        };
        let result = fdm.run();
        self.collect()?;
        result
    }

    fn op(&mut self, op: &str, args: &[&str]) -> Result<String, jsbsim::Error> {
        let resolver = self.resolver();
        let arg = |i: usize| args.get(i).copied().unwrap_or_default();
        let number_at = |i: usize| arg(i).parse::<f64>().unwrap_or(0.0);
        let Some(fdm) = self.fdm.as_mut() else {
            return Ok(" no-exec".to_owned());
        };
        match op {
            "script" => Ok(match fdm.load_script(&resolver, arg(0)) {
                Ok(()) => " ok".to_owned(),
                Err(e) => load_failure(e),
            }),
            "model" => {
                self.model = arg(0).to_owned();
                Ok(match fdm.load_model(&resolver, &self.aircraft, arg(0)) {
                    Ok(()) => " ok".to_owned(),
                    Err(e) => load_failure(e),
                })
            }
            "ic" => {
                let dir = format!("{}/{}", self.aircraft, self.model);
                let loaded = fdm.load_initial_conditions(&resolver, &dir, arg(0))?;
                Ok(format!(" {loaded}"))
            }
            "planet" => {
                fdm.load_planet_file(&resolver, arg(0), arg(1) != "0")?;
                Ok(" true".to_owned())
            }
            "set" => {
                fdm.set_property(arg(0), number_at(1))?;
                self.collect()?;
                Ok(" ok".to_owned())
            }
            "dt" => {
                fdm.set_dt(number_at(0));
                Ok(" ok".to_owned())
            }
            "runic" => {
                fdm.run_ic()?;
                let mode = fdm.trim_requested();
                if mode != TrimMode::None {
                    let mut trim = Trim::new(fdm, mode);
                    let success = trim.do_trim(fdm)?;
                    let mut report = trim.report(fdm);
                    report.success = success;
                    self.trims
                        .push_str(&record(fdm.models.exec.sim_time, &report));
                    let after = Self::print(fdm, &self.watched)?;
                    self.trims.push_str(&format!(" after{after}"));
                }
                self.print_now()
            }
            "run" => {
                let n = arg(0).parse::<i64>().unwrap_or(0);
                for _ in 0..n {
                    if !self.step()? {
                        break;
                    }
                }
                self.print_now()
            }
            "until" => {
                let t = number_at(0);
                while self
                    .fdm
                    .as_ref()
                    .is_some_and(|f| f.models.exec.sim_time < t)
                {
                    self.step()?;
                }
                self.print_now()
            }
            "print" => self.print_now(),
            "linearize" => {
                let lin = fdm.linearize()?;
                Ok(linearization(&lin) + &Self::print(fdm, &self.watched)?)
            }
            _ => Ok(" unknown".to_owned()),
        }
    }

    fn print_now(&mut self) -> Result<String, jsbsim::Error> {
        match self.fdm.as_mut() {
            Some(fdm) => Self::print(fdm, &self.watched),
            None => Ok(" no-exec".to_owned()),
        }
    }

    fn run(&mut self, op: &str, args: &[&str]) -> String {
        match op {
            "paths" => {
                self.root = args.first().copied().unwrap_or_default().to_owned();
                self.aircraft = args.get(1).copied().unwrap_or_default().to_owned();
                self.systems = args.get(2).copied().unwrap_or_default().to_owned();
                return " ok".to_owned();
            }
            "new" => {
                return match Fdm::new() {
                    Ok(mut fdm) => {
                        fdm.set_aircraft_path(&self.aircraft);
                        fdm.set_systems_path(&self.systems);
                        self.fdm = Some(fdm);
                        self.trims_seen = 0;
                        " ok".to_owned()
                    }
                    Err(e) => format!(" error {e}"),
                };
            }
            "watch" => {
                self.watched = args.iter().map(|s| (*s).to_owned()).collect();
                return " ok".to_owned();
            }
            _ => {}
        }
        if self.fdm.is_none() {
            return " no-exec".to_owned();
        }
        self.trims.clear();
        let result = match self.op(op, args) {
            Ok(s) => s,
            Err(e) => {
                if let Err(e) = self.collect() {
                    tracing::debug!("{op}: {e}");
                }
                match e {
                    jsbsim::Error::TrimFailed { message } => format!(" trim-failed {message}"),
                    jsbsim::Error::Unsupported { construct, path } => {
                        format!(" unsupported {construct} ({path})")
                    }
                    e => {
                        tracing::debug!("{op}: error: {e}");
                        " error".to_owned()
                    }
                }
            }
        };
        format!("{}{result}", self.trims)
    }
}

/// What the C++ probe's `Linearize` prints.
fn linearization(lin: &Linearization) -> String {
    let mut out = format!(" name {}", lin.aircraft_name);
    for (tag, v) in [("x", &lin.x), ("u", &lin.u), ("y", &lin.y)] {
        out.push_str(&format!(" {tag}"));
        for c in v {
            out.push_str(&format!(" {} {}", c.name(), c.unit()));
        }
    }
    for (tag, v) in [("x0", &lin.x0), ("u0", &lin.u0), ("y0", &lin.y0)] {
        out.push_str(&format!(" {tag}"));
        for x in v {
            out.push_str(&format!(" {}", number(*x)));
        }
    }
    for (tag, m) in [("A", &lin.a), ("B", &lin.b), ("C", &lin.c), ("D", &lin.d)] {
        out.push_str(&format!(" {tag}"));
        for x in m.iter().flatten() {
            out.push_str(&format!(" {}", number(*x)));
        }
    }
    out
}

fn load_failure(e: jsbsim::Error) -> String {
    match e {
        // GeographicLib throws out of LoadModel rather than returning false.
        jsbsim::Error::Geographic { message } => {
            tracing::debug!("error: {message}");
            " error".to_owned()
        }
        jsbsim::Error::Unsupported { construct, path } => {
            format!(" unsupported {construct} ({path})")
        }
        e => {
            tracing::debug!("load-error: {e}");
            " load-error".to_owned()
        }
    }
}

/// Runs the probe input.
pub fn run(input: &str, context: &ProbeContext) -> String {
    let mut probe = Probe {
        input_dir: context.input_dir.clone(),
        ..Probe::default()
    };
    let mut out = String::new();
    for line in input.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace();
        let op = words.next().unwrap_or_default();
        let args: Vec<&str> = words.collect();
        let result = probe.run(op, &args);
        out.push_str(&format!("{line} =>{result}\n"));
    }
    out
}
