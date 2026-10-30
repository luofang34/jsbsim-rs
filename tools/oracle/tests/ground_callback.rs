// SPDX-License-Identifier: LGPL-2.1-or-later
//! A ground callback is the terrain every contact, the friction solve and
//! the initial conditions see: gear_rover (oracle/scenarios) rests on a
//! moving deck, tripod_ground (oracle/derived) holds or slides on a slope
//! as its friction allows, and an FDM keeps the callback across a reset
//! and a planet load.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use jsbsim::Fdm;
use jsbsim::FileResolver;
use jsbsim::math::{ColumnVector3, E_X, E_Y, E_Z, Location, dot_product};
use jsbsim::{Contact, GroundCallback};

/// A plane through (a, 0, 0) at time 0, the planet's surface at latitude
/// and longitude 0, with unit normal `normal`, moving at `velocity`.
struct Plane {
    a: f64,
    b: f64,
    normal: ColumnVector3,
    velocity: ColumnVector3,
    ellipses: Arc<Mutex<Vec<(f64, f64)>>>,
}

impl Plane {
    fn new(fdm: &Fdm, normal: ColumnVector3, velocity: ColumnVector3) -> Self {
        Self {
            a: fdm.models.inertial.a,
            b: fdm.models.inertial.b,
            normal,
            velocity,
            ellipses: Arc::default(),
        }
    }
}

#[cfg(test)]
impl GroundCallback for Plane {
    fn ag_level(&self, time: f64, location: &Location) -> Contact {
        let origin = ColumnVector3::new(self.a, 0.0, 0.0) + self.velocity * time;
        let r = location.ecef();
        let agl = dot_product(r - origin, self.normal);
        let mut point = Location::from_ecef(r - agl * self.normal);
        point.set_ellipse(self.a, self.b);
        point.cache_derived();
        Contact::new(agl, point, self.normal).with_motion(self.velocity, ColumnVector3::zero())
    }

    fn set_ellipse(&mut self, semimajor: f64, semiminor: f64) {
        self.ellipses.lock().unwrap().push((semimajor, semiminor));
    }
}

#[cfg(test)]
fn oracle_dir(sub: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .join(sub)
}

/// `model` of `root`'s aircraft at latitude and longitude 0 on
/// `callback`, with its initial conditions set but not run.
#[cfg(test)]
fn vehicle(
    root: &str,
    model: &str,
    callback: Option<Box<dyn GroundCallback>>,
    ic: &[(&str, f64)],
) -> Fdm {
    let mut fdm = Fdm::new().unwrap();
    if let Some(callback) = callback {
        fdm.set_ground_callback(callback);
    }
    fdm.load_model(&FileResolver::new(oracle_dir(root)), "aircraft", model)
        .unwrap();
    for (name, value) in [("ic/lat-geod-deg", 0.0), ("ic/long-gc-deg", 0.0)]
        .iter()
        .chain(ic)
    {
        fdm.set_property(name, *value).unwrap();
    }
    fdm
}

#[cfg(test)]
fn ecef(fdm: &mut Fdm) -> ColumnVector3 {
    ColumnVector3::new(
        fdm.get_property("position/ecef-x-ft").unwrap(),
        fdm.get_property("position/ecef-y-ft").unwrap(),
        fdm.get_property("position/ecef-z-ft").unwrap(),
    )
}

#[cfg(test)]
fn run_for(fdm: &mut Fdm, seconds: f64) {
    let end = fdm.models.exec.sim_time + seconds;
    while fdm.models.exec.sim_time < end {
        assert!(fdm.run().unwrap());
    }
}

/// gear_rover rolling north at 20 ft/s on its wheels, every brake on.
#[cfg(test)]
const ROLLING_NORTH: &[(&str, f64)] = &[
    ("ic/psi-true-deg", 0.0),
    ("ic/u-fps", 20.0),
    ("ic/h-agl-ft", 2.39),
    ("fcs/left-brake-cmd-norm", 1.0),
    ("fcs/right-brake-cmd-norm", 1.0),
    ("fcs/center-brake-cmd-norm", 1.0),
];

/// A level deck moving north at 20 ft/s.
#[cfg(test)]
fn deck_north() -> (Box<Plane>, ColumnVector3) {
    let velocity = ColumnVector3::new(0.0, 0.0, 20.0);
    let up = ColumnVector3::new(1.0, 0.0, 0.0);
    let fdm = Fdm::new().unwrap();
    (Box::new(Plane::new(&fdm, up, velocity)), velocity)
}

#[test]
fn a_vehicle_moving_with_a_deck_stays_at_rest_on_it() {
    let (deck, deck_velocity) = deck_north();
    let mut fdm = vehicle("scenarios", "gear_rover", Some(deck), ROLLING_NORTH);
    fdm.run_ic().unwrap();
    // Out of the initial compression's transient first.
    run_for(&mut fdm, 1.0);
    let (t0, p0) = (fdm.models.exec.sim_time, ecef(&mut fdm));
    run_for(&mut fdm, 5.0);
    let (t1, p1) = (fdm.models.exec.sim_time, ecef(&mut fdm));
    let drift = (p1 - p0) - deck_velocity * (t1 - t0);
    assert!(
        drift[E_Y].abs() < 1e-3 && drift[E_Z].abs() < 1e-3,
        "the rover moved {drift:?} ft relative to the deck"
    );
    assert!(drift[E_X].abs() < 1e-2, "the rover sank or rose {drift:?}");
    let vn = fdm.get_property("velocities/v-north-fps").unwrap();
    assert!((vn - 20.0).abs() < 1e-4, "v-north {vn} ft/s");

    // The same brakes stop it on the default, stationary ground.
    let mut fdm = vehicle("scenarios", "gear_rover", None, ROLLING_NORTH);
    fdm.run_ic().unwrap();
    run_for(&mut fdm, 6.0);
    let vn = fdm.get_property("velocities/v-north-fps").unwrap();
    assert!(vn.abs() < 0.1, "on still ground v-north is {vn} ft/s");
}

/// How far tripod_ground slides down a plane tilted by `degrees` (lower
/// to the north) between 2 s and 6 s.
#[cfg(test)]
fn slope_drift_ft(degrees: f64) -> f64 {
    let (s, c) = degrees.to_radians().sin_cos();
    let fdm = Fdm::new().unwrap();
    let normal = ColumnVector3::new(c, 0.0, s);
    let slope = Box::new(Plane::new(&fdm, normal, ColumnVector3::zero()));
    // tripod_ground stands on three contacts whose static and dynamic
    // friction coefficients are 0.1.
    let mut fdm = vehicle(
        "derived",
        "tripod_ground",
        Some(slope),
        &[
            ("ic/psi-true-deg", 0.0),
            ("ic/theta-deg", -degrees),
            ("ic/h-agl-ft", 0.1),
        ],
    );
    fdm.run_ic().unwrap();
    run_for(&mut fdm, 2.0);
    let p0 = ecef(&mut fdm);
    run_for(&mut fdm, 4.0);
    let moved = ecef(&mut fdm) - p0;
    // The downhill direction along the plane is (-sin, 0, cos).
    moved[E_Z] * c - moved[E_X] * s
}

#[test]
fn friction_holds_below_the_friction_angle_and_slides_above_it() {
    // atan(0.1) is 5.7 degrees.
    let held = slope_drift_ft(4.0);
    assert!(held.abs() < 0.01, "on 4 degrees the tripod slid {held} ft");
    let slid = slope_drift_ft(8.0);
    assert!(slid > 5.0, "on 8 degrees the tripod slid only {slid} ft");
}

#[test]
fn a_reset_and_a_planet_load_keep_the_installed_callback() {
    let (deck, deck_velocity) = deck_north();
    let mut fdm = vehicle("scenarios", "gear_rover", Some(deck), ROLLING_NORTH);
    fdm.run_ic().unwrap();
    run_for(&mut fdm, 1.0);
    fdm.reset_to_initial_conditions(jsbsim::ResetMask::empty())
        .unwrap();
    assert!(fdm.models.inertial.has_custom_ground_callback());
    assert_eq!(fdm.models.propagate.local_terrain_velocity, deck_velocity);
    run_for(&mut fdm, 0.5);
    assert_eq!(fdm.get_property("gear/wow").unwrap(), 1.0);

    let mut fdm = Fdm::new().unwrap();
    let up = ColumnVector3::new(1.0, 0.0, 0.0);
    let plane = Plane::new(&fdm, up, ColumnVector3::zero());
    let ellipses = Arc::clone(&plane.ellipses);
    fdm.set_ground_callback(Box::new(plane));
    let resolver = FileResolver::new(oracle_dir("derived"));
    fdm.load_model(&resolver, "aircraft", "tripod_moon")
        .unwrap();
    assert!(fdm.models.inertial.has_custom_ground_callback());
    let (a, b) = (fdm.models.inertial.a, fdm.models.inertial.b);
    assert_eq!(*ellipses.lock().unwrap(), [(a, b)]);
    assert!(a < 6e6, "the Moon's equatorial radius is {a} ft");
}
