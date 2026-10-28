// SPDX-License-Identifier: LGPL-2.1-or-later
//! Auxiliary probe; twin of `tools/oracle/cpp/probe_auxiliary.cpp`.

use jsbsim::c_numeric::format_g;
use jsbsim::fdm::Fdm;
use jsbsim::math::{ColumnVector3, Location, Matrix33};
use jsbsim::models::auxiliary::{Inputs, mach_from_impact_pressure, pitot_total_pressure};

const PROPERTIES: [&str; 60] = [
    "propulsion/tat-r",
    "propulsion/tat-c",
    "propulsion/pt-lbs_sqft",
    "velocities/vc-fps",
    "velocities/vc-kts",
    "velocities/ve-fps",
    "velocities/ve-kts",
    "velocities/vtrue-fps",
    "velocities/vtrue-kts",
    "velocities/machU",
    "velocities/p-aero-rad_sec",
    "velocities/q-aero-rad_sec",
    "velocities/r-aero-rad_sec",
    "velocities/phidot-rad_sec",
    "velocities/thetadot-rad_sec",
    "velocities/psidot-rad_sec",
    "velocities/u-aero-fps",
    "velocities/v-aero-fps",
    "velocities/w-aero-fps",
    "velocities/vt-fps",
    "velocities/mach",
    "velocities/vg-fps",
    "accelerations/a-pilot-x-ft_sec2",
    "accelerations/a-pilot-y-ft_sec2",
    "accelerations/a-pilot-z-ft_sec2",
    "accelerations/n-pilot-x-norm",
    "accelerations/n-pilot-y-norm",
    "accelerations/n-pilot-z-norm",
    "accelerations/Nx",
    "accelerations/Ny",
    "accelerations/Nz",
    "forces/load-factor",
    "aero/alpha-rad",
    "aero/beta-rad",
    "aero/mag-beta-rad",
    "aero/alpha-deg",
    "aero/beta-deg",
    "aero/mag-beta-deg",
    "aero/Re",
    "aero/qbar-psf",
    "aero/qbarUW-psf",
    "aero/qbarUV-psf",
    "aero/alphadot-rad_sec",
    "aero/betadot-rad_sec",
    "aero/alphadot-deg_sec",
    "aero/betadot-deg_sec",
    "aero/h_b-cg-ft",
    "aero/h_b-mac-ft",
    "flight-path/gamma-rad",
    "flight-path/gamma-deg",
    "flight-path/psi-gt-rad",
    "position/distance-from-start-lon-mt",
    "position/distance-from-start-lat-mt",
    "position/distance-from-start-mag-mt",
    "position/vrp-gc-latitude_deg",
    "position/vrp-longitude_deg",
    "position/vrp-radius-ft",
    "position/from-start-neu-n-ft",
    "position/from-start-neu-e-ft",
    "position/from-start-neu-u-ft",
];

#[derive(Default)]
struct Out(String);

impl Out {
    fn num(&mut self, tag: &str, v: f64) {
        self.0.push(' ');
        self.0.push_str(tag);
        self.0.push(' ');
        self.0.push_str(&format_g(v, 17));
    }

    fn vec(&mut self, tag: &str, v: ColumnVector3) {
        for i in 1..=3 {
            self.num(&format!("{tag}{i}"), v[i]);
        }
    }

    fn mat(&mut self, tag: &str, m: &Matrix33) {
        for r in 1..=3 {
            for c in 1..=3 {
                self.num(&format!("{tag}{r}{c}"), m[(r, c)]);
            }
        }
    }
}

fn print(fdm: &mut Fdm) -> Result<String, jsbsim::Error> {
    let mut o = Out::default();
    for p in PROPERTIES {
        let v = fdm.get_property(p)?;
        o.num(p, v);
    }
    let x = &fdm.models.auxiliary;
    o.mat("tw2b", &x.tw2b);
    o.mat("tb2w", &x.tb2w);
    o.vec("ncg", x.loads.ncg);
    o.vec("nwcg", x.loads.nwcg);
    Ok(o.0)
}

fn scalar<'a>(i: &'a mut Inputs, name: &str) -> Option<&'a mut f64> {
    Some(match name {
        "pressure" => &mut i.air.pressure,
        "density" => &mut i.air.density,
        "temperature" => &mut i.air.temperature,
        "soundspeed" => &mut i.air.sound_speed,
        "nu" => &mut i.air.kinematic_viscosity,
        "agl" => &mut i.distance_agl,
        "span" => &mut i.constants.wingspan,
        "chord" => &mut i.constants.wingchord,
        "mass" => &mut i.mass,
        "costht" => &mut i.cos_tht,
        "sintht" => &mut i.sin_tht,
        "cosphi" => &mut i.cos_phi,
        "sinphi" => &mut i.sin_phi,
        _ => return None,
    })
}

fn vector<'a>(i: &'a mut Inputs, name: &str) -> Option<&'a mut ColumnVector3> {
    Some(match name {
        "pqr" => &mut i.pqr,
        "pqri" => &mut i.pqri,
        "pqridot" => &mut i.pqridot,
        "uvw" => &mut i.uvw,
        "uvwdot" => &mut i.uvwdot,
        "vel" => &mut i.vel,
        "accel" => &mut i.body_accel,
        "eye" => &mut i.to_eye_pt,
        "rp" => &mut i.rp_body,
        "vrp" => &mut i.vrp_body,
        "fw" => &mut i.fw,
        "wind" => &mut i.total_wind_ned,
        "turb" => &mut i.turb_pqr,
        _ => return None,
    })
}

fn matrix<'a>(i: &'a mut Inputs, name: &str) -> Option<&'a mut Matrix33> {
    Some(match name {
        "tl2b" => &mut i.tl2b,
        "tb2l" => &mut i.tb2l,
        _ => return None,
    })
}

/// Every input zero but the planet constants.
fn zero(i: &mut Inputs) {
    let c = i.constants;
    *i = Inputs::default();
    i.constants.standard_gravity = c.standard_gravity;
    i.constants.std_day_sl_soundspeed = c.std_day_sl_soundspeed;
}

fn geodetic(fdm: &Fdm, lon: f64, lat: f64, h: f64) -> Location {
    let mut l = Location::default();
    l.set_ellipse(fdm.models.inertial.a, fdm.models.inertial.b);
    l.set_position_geodetic(lon, lat, h);
    l
}

/// The argument at `i` as a number, 0 when missing or malformed.
fn num(args: &[&str], i: usize) -> f64 {
    args.get(i).and_then(|s| s.parse().ok()).unwrap_or_default()
}

/// The input-setting operations; `None` for any other.
fn set_input(fdm: &mut Fdm, op: &str, args: &[&str]) -> Option<&'static str> {
    let name = args.first().copied().unwrap_or_default();
    let input = &mut fdm.models.auxiliary.input;
    let known = match op {
        "zero" => {
            zero(input);
            true
        }
        "s" => scalar(input, name).map(|p| *p = num(args, 1)).is_some(),
        "v" => vector(input, name)
            .map(|p| *p = ColumnVector3::new(num(args, 1), num(args, 2), num(args, 3)))
            .is_some(),
        "m" => matrix(input, name)
            .map(|m| {
                for r in 1..=3 {
                    for c in 1..=3 {
                        m[(r, c)] = num(args, 3 * (r - 1) + c);
                    }
                }
            })
            .is_some(),
        "loc" => {
            let l = geodetic(fdm, num(args, 0), num(args, 1), num(args, 2));
            fdm.models.auxiliary.input.location = l;
            true
        }
        "ic" => {
            let start = geodetic(fdm, 0.0, 0.0, 0.0);
            let (a, b) = (fdm.models.inertial.a, fdm.models.inertial.b);
            fdm.models.auxiliary.set_initial_state(&start, a, b);
            true
        }
        _ => return None,
    };
    Some(if known { " ok" } else { " unknown" })
}

fn step(fdm: &mut Fdm, op: &str, args: &[&str]) -> Result<String, jsbsim::Error> {
    if let Some(result) = set_input(fdm, op, args) {
        return Ok(result.to_owned());
    }
    match op {
        "run" | "hold" => {
            fdm.models.auxiliary.run(op == "hold");
            return print(fdm);
        }
        "init" => {
            fdm.models.auxiliary.init_model();
            return print(fdm);
        }
        _ => {}
    }
    let x = &fdm.models.auxiliary;
    let (a, b) = (num(args, 0), num(args, 1));
    let mut o = Out::default();
    match op {
        "pitot" => o.num("pt", pitot_total_pressure(a, b)),
        "machqc" => o.num("M", mach_from_impact_pressure(a, b)),
        "vcas" => o.num("vc", x.vcalibrated_from_mach(a, b)),
        "machvc" => o.num("M", x.mach_from_vcalibrated(a, b)),
        "heading" => o.num("psi", x.input.location.heading_to(a, b)),
        _ => return Ok(" unknown".to_owned()),
    }
    Ok(o.0)
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
