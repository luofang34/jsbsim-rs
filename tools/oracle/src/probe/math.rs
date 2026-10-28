// SPDX-License-Identifier: LGPL-2.1-or-later
//! Math probe; twin of `tools/oracle/cpp/probe_math.cpp`.

use jsbsim::c_numeric::format_g;
use jsbsim::math::{
    Axis, ColumnVector3, E_X, E_Y, E_Z, Location, Matrix33, Quaternion, dot_product,
};

#[derive(Default)]
struct Out(String);

impl Out {
    fn num(&mut self, v: f64) {
        self.0.push(' ');
        self.0.push_str(&format_g(v, 17));
    }
    fn tag(&mut self, t: &str) {
        self.0.push(' ');
        self.0.push_str(t);
    }
    fn tagged(&mut self, t: &str, v: f64) {
        self.tag(t);
        self.num(v);
    }
    fn vec(&mut self, t: &str, v: ColumnVector3) {
        self.tag(t);
        for x in v.to_array() {
            self.num(x);
        }
    }
    fn mat(&mut self, t: &str, m: &Matrix33) {
        self.tag(t);
        for r in Axis::ALL {
            for c in Axis::ALL {
                self.num(m[(r, c)]);
            }
        }
    }
    fn quat(&mut self, t: &str, q: &Quaternion) {
        self.tag(t);
        for x in q.to_array() {
            self.num(x);
        }
    }
}

fn vector(o: &mut Out, a: &Args) {
    let u = ColumnVector3::new(a[0], a[1], a[2]);
    let v = ColumnVector3::new(a[3], a[4], a[5]);
    o.vec("sum", u + v);
    o.vec("diff", u - v);
    o.vec("cross", u.cross(v));
    o.tagged("dot", dot_product(u, v));
    o.tagged("mag", u.magnitude());
    o.tagged("mag12", u.magnitude2(E_X, E_Y));
    o.tagged("mag23", u.magnitude2(E_Y, E_Z));
    let mut n = u;
    o.vec("norm", *n.normalize());
    o.vec("scaled", 2.5 * u);
    o.vec("div3", u / 3.0);
    o.vec("div0", u / 0.0);
    let mut w = u;
    w /= v[E_X];
    o.vec("divassign", w);
}

fn matrix(o: &mut Out, a: &Args) {
    let m = Matrix33::new(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8]);
    let v = ColumnVector3::new(1.25, -2.5, 3.75);
    o.tagged("det", m.determinant());
    o.mat("inv", &m.inverse());
    o.mat("tr", &m.transposed());
    o.mat("sq", &(m * m));
    let mut p = m;
    p *= m.transposed();
    o.mat("mulassign", &p);
    o.mat("sum", &(m + m.transposed()));
    o.mat("diff", &(m - m.transposed()));
    o.mat("scaled", &(0.3 * m));
    o.mat("div7", &(m / 7.0));
    o.vec("mv", m * v);
    o.vec("euler", m.euler());
    o.quat("quat", &m.quaternion());
}

fn quaternion(o: &mut Out, q: &Quaternion) {
    let pqr = ColumnVector3::new(0.1, -0.2, 0.3);
    o.quat("q", q);
    o.mat("T", &q.t());
    o.mat("Tinv", &q.t_inv());
    o.vec("euler", q.euler());
    o.vec("eulerdeg", q.euler_deg());
    o.tag("sin");
    for i in Axis::ALL {
        o.num(q.sin_euler(i));
    }
    o.tag("cos");
    for i in Axis::ALL {
        o.num(q.cos_euler(i));
    }
    o.quat("qdot", &q.q_dot(pqr));
    o.quat("inv", &q.inverse());
    o.quat("conj", &q.conjugate());
    o.quat("sq", &(q * q));
    o.quat("sum", &(q + &q.conjugate()));
    o.quat("fromT", &Quaternion::from_matrix(&q.t()));
    let mut n = 3.0 * q;
    n.normalize();
    o.quat("renorm", &n);
    o.tagged("mag", q.magnitude());
}

fn location(o: &mut Out, l: &Location, ellipse: bool) {
    let lvec = ColumnVector3::new(100.0, -250.0, 30.0);
    o.tagged("lon", l.longitude());
    o.tagged("londeg", l.longitude_deg());
    o.tagged("lat", l.latitude());
    o.tagged("latdeg", l.latitude_deg());
    o.tagged("radius", l.radius());
    o.tagged("sincoslon", l.sin_longitude());
    o.num(l.cos_longitude());
    if ellipse {
        o.tagged("geodlat", l.geod_latitude_rad());
        o.tagged("geodlatdeg", l.geod_latitude_deg());
        o.tagged("geodalt", l.geod_altitude());
    }
    o.tagged("sealevel", l.sea_level_radius());
    o.mat("tec2l", &l.tec2l());
    o.mat("tl2ec", &l.tl2ec());
    let moved = l.local_to_location(lvec);
    o.vec("l2loc", moved.ecef());
    o.vec("loc2l", l.location_to_local(moved.ecef()));
    let sum = l.add(l);
    o.tagged("sumradius", sum.radius());
    if ellipse {
        o.tagged("sumgeodalt", sum.geod_altitude());
    }
    let half = l.scale(0.5);
    if ellipse {
        o.tagged("halfgeodlat", half.geod_latitude_rad());
    }
}

/// The operands of one operation; missing ones read as NaN.
type Args = [f64; 9];

fn run_op(o: &mut Out, op: &str, args: &[f64]) {
    let mut a: Args = [f64::NAN; 9];
    for (slot, &v) in a.iter_mut().zip(args) {
        *slot = v;
    }
    let a = &a;
    match op {
        "vec" => vector(o, a),
        "mat" => matrix(o, a),
        "quat" => quaternion(o, &Quaternion::from_euler(a[0], a[1], a[2])),
        "qaxis" => {
            // `FGQuaternion(idx, angle)` treats any index but ePhi or eTht as
            // ePsi.
            let axis = Axis::from_index1(a[0] as usize).unwrap_or(Axis::Z);
            quaternion(o, &Quaternion::from_axis_index(axis, a[1]));
        }
        "qangleaxis" => quaternion(
            o,
            &Quaternion::from_angle_axis(a[0], ColumnVector3::new(a[1], a[2], a[3])),
        ),
        "qexp" => quaternion(o, &Quaternion::exp(ColumnVector3::new(a[0], a[1], a[2]))),
        "loc" => {
            let mut l = Location::from_ecef(ColumnVector3::new(a[0], a[1], a[2]));
            if args.len() == 5 {
                l.set_ellipse(a[3], a[4]);
            }
            location(o, &l, args.len() == 5);
        }
        "geod" => {
            let mut l = Location::default();
            l.set_ellipse(a[3], a[4]);
            l.set_position_geodetic(a[0], a[1], a[2]);
            o.vec("ecef", l.ecef());
            location(o, &l, true);
        }
        "locset" => {
            let mut l = Location::from_ecef(ColumnVector3::new(a[0], a[1], a[2]));
            l.set_longitude(a[3]);
            o.vec("setlon", l.ecef());
            l.set_latitude(a[4]);
            o.vec("setlat", l.ecef());
            l.set_radius(a[5]);
            o.vec("setrad", l.ecef());
            o.vec(
                "spherical",
                Location::from_spherical(a[3], a[4], a[5]).ecef(),
            );
        }
        _ => o.tag("unknown"),
    }
}

/// Executes the operations in `input`, one per line.
pub fn run(input: &str) -> String {
    let mut out = String::new();
    for line in input.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace();
        let op = words.next().unwrap_or_default();
        let args: Vec<f64> = words.filter_map(|w| w.parse().ok()).collect();
        let mut o = Out::default();
        run_op(&mut o, op, &args);
        out.push_str(&format!("{line} =>{}\n", o.0));
    }
    out
}
