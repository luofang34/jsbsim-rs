// SPDX-License-Identifier: LGPL-2.1-or-later
//! Random probe; twin of `tools/oracle/cpp/probe_random.cpp`.

use std::sync::Arc;

use jsbsim::Fdm;
use jsbsim::oracle_support::c_numeric::format_g;
use jsbsim::oracle_support::random::{RandomNumberGenerator, SharedGenerator};

#[derive(Default)]
struct Probe {
    fdm: Option<Fdm>,
    generator: Option<SharedGenerator>,
}

fn num(out: &mut String, v: f64) {
    out.push(' ');
    out.push_str(&format_g(v, 17));
}

/// `>>` into an `int`: a word that does not start with a number reads 0.
fn int(words: &[&str], i: usize) -> i32 {
    let word = words.get(i).copied().unwrap_or_default();
    jsbsim::oracle_support::c_numeric::atoi(word)
}

fn draw(out: &mut String, g: &RandomNumberGenerator, kind: char) {
    match kind {
        'u' => num(out, g.uniform()),
        'n' => num(out, g.normal()),
        _ => out.push_str(" bad"),
    }
}

impl Probe {
    fn run(&mut self, words: &[&str]) -> String {
        let op = words.first().copied().unwrap_or_default();
        match op {
            "new" => {
                // `RandomNumberGenerator(unsigned int)` from an `int`.
                self.generator = Some(Arc::new(RandomNumberGenerator::new(int(words, 1) as u32)));
                return " ok".to_owned();
            }
            "exec" => {
                return match Fdm::new() {
                    Ok(fdm) => {
                        self.generator = Some(fdm.models.exec.rng.clone());
                        self.fdm = Some(fdm);
                        " ok".to_owned()
                    }
                    Err(e) => format!(" error {e}"),
                };
            }
            "set" | "get" => {
                let Some(fdm) = self.fdm.as_mut() else {
                    return " no-exec".to_owned();
                };
                let name = words.get(1).copied().unwrap_or_default();
                let v = words.get(2).and_then(|w| w.parse().ok()).unwrap_or(0.0);
                let result = if op == "set" {
                    fdm.set_property(name, v)
                        .and_then(|_| fdm.get_property(name))
                } else {
                    fdm.get_property(name)
                };
                let mut out = String::new();
                match result {
                    Ok(v) => num(&mut out, v),
                    Err(e) => return format!(" error {e}"),
                }
                return out;
            }
            _ => {}
        }
        let Some(g) = self.generator.as_deref() else {
            return " no-generator".to_owned();
        };
        let mut out = String::new();
        match op {
            "seed" => {
                g.seed(int(words, 1) as u32);
                out.push_str(" ok");
            }
            "uniform" | "normal" => {
                let kind = if op == "uniform" { 'u' } else { 'n' };
                for _ in 0..int(words, 1) {
                    draw(&mut out, g, kind);
                }
            }
            "draws" => {
                for c in words.get(1).copied().unwrap_or_default().chars() {
                    draw(&mut out, g, c);
                }
            }
            _ => out.push_str(" unknown"),
        }
        out
    }
}

/// Executes the operations in `input`, one per line.
pub fn run(input: &str) -> String {
    let mut probe = Probe::default();
    let mut out = String::new();
    for line in input.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let result = probe.run(&words);
        out.push_str(&format!("{line} =>{result}\n"));
    }
    out
}
