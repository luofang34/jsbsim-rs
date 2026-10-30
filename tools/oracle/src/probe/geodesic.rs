// SPDX-License-Identifier: LGPL-2.1-or-later
//! Geodesic probe; twin of `tools/oracle/cpp/probe_geodesic.cpp`.

use jsbsim::Fdm;
use jsbsim::math::Location;
use jsbsim::oracle_support::c_numeric::format_g;
use jsbsim::oracle_support::xml::parse_document;

use super::ProbeContext;

fn num(out: &mut String, tag: &str, v: f64) {
    out.push(' ');
    out.push_str(tag);
    out.push(' ');
    out.push_str(&format_g(v, 17));
}

#[derive(Default)]
struct Probe {
    axes: Option<(f64, f64)>,
}

impl Probe {
    fn axes(&self) -> String {
        let mut out = String::new();
        if let Some((a, b)) = self.axes {
            num(&mut out, "a", a);
            num(&mut out, "b", b);
        }
        out
    }

    fn wgs84(&mut self) -> String {
        match Fdm::new() {
            Ok(fdm) => {
                self.axes = Some((fdm.models.inertial.a, fdm.models.inertial.b));
                self.axes()
            }
            Err(e) => format!(" error {e}"),
        }
    }

    fn planet(&mut self, kind: &str, rel: &str, context: &ProbeContext) -> String {
        self.axes = None;
        let path = match kind {
            "file" => context.jsbsim_root.clone().unwrap_or_default().join(rel),
            _ => context.input_dir.join(rel),
        };
        let mut fdm = match Fdm::new() {
            Ok(fdm) => fdm,
            Err(e) => return format!(" error {e}"),
        };
        let Ok(bytes) = std::fs::read(&path) else {
            return " missing".to_owned();
        };
        let loaded = parse_document(&bytes, rel)
            .map_err(jsbsim::Error::from)
            .and_then(|doc| {
                let el = if doc.name() == "planet" {
                    Some(&doc)
                } else {
                    doc.find_element("planet")
                };
                el.map(|el| fdm.models.inertial.load(el)).transpose()
            });
        match loaded {
            Ok(Some(())) => {
                self.axes = Some((fdm.models.inertial.a, fdm.models.inertial.b));
                self.axes()
            }
            Ok(None) => " no-planet".to_owned(),
            Err(e) => {
                tracing::debug!("load-error: {e}");
                " load-error".to_owned()
            }
        }
    }

    /// `pair`, `anti`, `tiny`, `meridian` and `same`.
    fn pair(&self, op: &str, args: &[&str]) -> String {
        let Some((a, b)) = self.axes else {
            return " no-ellipse".to_owned();
        };
        let arg =
            |i: usize| -> f64 { args.get(i).and_then(|s| s.parse().ok()).unwrap_or_default() };
        let mut l = Location::default();
        l.set_ellipse(a, b);
        l.set_position_geodetic(arg(0), arg(1), arg(2));
        let (lon2, lat2) = match op {
            "same" => (l.longitude(), l.geod_latitude_rad()),
            "meridian" => (arg(3) + l.longitude(), arg(4)),
            _ => (arg(3), arg(4)),
        };
        let mut out = String::new();
        num(&mut out, "lon1", l.longitude());
        num(&mut out, "lat1", l.geod_latitude_rad());
        num(&mut out, "s", l.distance_to(lon2, lat2));
        num(&mut out, "psi", l.heading_to(lon2, lat2));
        out
    }

    fn run(&mut self, words: &[&str], context: &ProbeContext) -> String {
        let arg = |i: usize| words.get(i).copied().unwrap_or_default();
        match arg(0) {
            "wgs84" => self.wgs84(),
            "planet" => self.planet(arg(1), arg(2), context),
            op @ ("pair" | "anti" | "tiny" | "meridian" | "same") => {
                self.pair(op, words.get(1..).unwrap_or_default())
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
        out.push_str(line);
        out.push_str(" =>");
        out.push_str(&probe.run(&words, context));
        out.push('\n');
    }
    out
}
