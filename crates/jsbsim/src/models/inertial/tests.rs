// SPDX-License-Identifier: LGPL-2.1-or-later

use std::sync::{Arc, Mutex};

use super::{Contact, DefaultGroundCallback, GroundCallback, Inertial};
use crate::math::{ColumnVector3, E_X, E_Y, E_Z, Location};

/// What a [`Recording`] was given.
#[derive(Debug, Default)]
struct Log {
    times: Vec<f64>,
    elevations: Vec<f64>,
    ellipses: Vec<(f64, f64)>,
}

/// Terrain at x = 100 ft, moving and turning, that logs its calls.
struct Recording(Arc<Mutex<Log>>);

impl GroundCallback for Recording {
    fn ag_level(&self, time: f64, location: &Location) -> Contact {
        self.0.lock().unwrap().times.push(time);
        Contact {
            agl: location.entry(E_X) - 100.0,
            point: Location::from_ecef(ColumnVector3::new(100.0, 0.0, 0.0)),
            normal: ColumnVector3::new(1.0, 0.0, 0.0),
            velocity: ColumnVector3::new(0.0, 2.0, 0.0),
            angular_velocity: ColumnVector3::new(0.0, 0.0, 3.0),
        }
    }

    fn set_terrain_elevation(&mut self, elevation: f64) {
        self.0.lock().unwrap().elevations.push(elevation);
    }

    fn set_ellipse(&mut self, semimajor: f64, semiminor: f64) {
        self.0.lock().unwrap().ellipses.push((semimajor, semiminor));
    }
}

fn recording() -> (Box<Recording>, Arc<Mutex<Log>>) {
    let log = Arc::new(Mutex::new(Log::default()));
    (Box::new(Recording(Arc::clone(&log))), log)
}

fn bits(c: &Contact) -> Vec<u64> {
    let p = c.point.ecef();
    [
        c.agl,
        p[E_X],
        p[E_Y],
        p[E_Z],
        c.normal[E_X],
        c.normal[E_Y],
        c.normal[E_Z],
        c.velocity.magnitude(),
        c.angular_velocity.magnitude(),
        c.point.geod_altitude(),
        c.point.radius(),
    ]
    .map(f64::to_bits)
    .to_vec()
}

#[test]
fn the_default_callback_is_flat_terrain_at_the_elevation() {
    let mut inertial = Inertial::default();
    inertial.set_terrain_elevation(250.0);
    let mut location = Location::default();
    location.set_ellipse(inertial.a, inertial.b);
    location.set_position_geodetic(0.3, 0.7, 1250.0);
    let contact = inertial.contact(&location);
    assert!((contact.agl - 1000.0).abs() < 1e-6);
    assert!((contact.point.geod_altitude() - 250.0).abs() < 1e-6);
    assert_eq!(contact.velocity, ColumnVector3::zero());
    assert_eq!(contact.angular_velocity, ColumnVector3::zero());
    let mut flat = DefaultGroundCallback::new(inertial.a, inertial.b);
    flat.set_terrain_elevation(250.0);
    assert_eq!(flat.terrain_elevation(), 250.0);
    assert_eq!(bits(&flat.ag_level(5.0, &location)), bits(&contact));
}

#[test]
fn an_installed_callback_answers_every_query_at_the_time_given() {
    let mut inertial = Inertial::default();
    inertial.set_time(7.0);
    let (callback, log) = recording();
    inertial.set_ground_callback(callback);
    assert!(inertial.has_custom_ground_callback());
    let location = Location::from_ecef(ColumnVector3::new(130.0, 0.0, 0.0));
    // Like a newly made FGGroundCallback, it starts at time 0.
    assert_eq!(inertial.altitude_agl(&location), 30.0);
    inertial.set_time(2.5);
    let contact = inertial.contact(&location);
    assert_eq!(contact.velocity, ColumnVector3::new(0.0, 2.0, 0.0));
    assert_eq!(contact.angular_velocity, ColumnVector3::new(0.0, 0.0, 3.0));
    inertial.set_terrain_elevation(500.0);
    assert_eq!(inertial.altitude_agl(&location), 30.0);
    let log = log.lock().unwrap();
    assert_eq!(log.times, [0.0, 2.5, 2.5]);
    assert_eq!(log.elevations, [500.0]);
}

#[test]
fn loading_a_planet_gives_the_callback_its_axes() {
    let mut inertial = Inertial::default();
    let (callback, log) = recording();
    inertial.set_ground_callback(callback);
    let xml = "<planet name='p'><semimajor_axis unit='FT'>1000</semimajor_axis>\
               <semiminor_axis unit='FT'>900</semiminor_axis></planet>";
    let el = crate::xml::parse_document(xml.as_bytes(), "planet.xml").unwrap();
    inertial.load(&el).unwrap();
    assert!(inertial.has_custom_ground_callback());
    assert_eq!(log.lock().unwrap().ellipses, [(1000.0, 900.0)]);
}
