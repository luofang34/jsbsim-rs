// SPDX-License-Identifier: LGPL-2.1-or-later
// FGGroundCallback: Copyright (C) 2004 Mathias Froehlich.
// Derived from JSBSim src/input_output/FGGroundCallback.{h,cpp}.
//
//! The ground callback: what the terrain under a location is, as
//! `FGGroundCallback` reports it, and the default flat terrain of
//! `FGDefaultGroundCallback`.

use crate::math::{ColumnVector3, Location};

/// What a ground callback reports about the terrain under a location
/// (the out-parameters and the return value of `GetAGLevel`).
#[derive(Debug, Clone, Copy)]
pub struct Contact {
    /// Height of the location above the terrain, ft.
    pub agl: f64,
    /// Terrain point below the location.
    pub point: Location,
    /// Terrain normal, ECEF.
    pub normal: ColumnVector3,
    /// Surface velocity at the queried point, ECEF, ft/s, including any
    /// rotation of the surface: the gear's wheel velocity uses this alone.
    pub velocity: ColumnVector3,
    /// Terrain angular velocity, ECEF, rad/s. Only the friction solve
    /// reads it, from the query below the centre of gravity; the gear
    /// ignores it.
    pub angular_velocity: ColumnVector3,
}

/// `FGGroundCallback`: the terrain the vehicle's contacts and the
/// altitude above ground are measured against.
///
/// Install one with [`Fdm::set_ground_callback`](crate::fdm::Fdm::set_ground_callback);
/// the [`Inertial`](super::Inertial) model owns it and queries it with the
/// simulation time it was last given.
///
/// # Contract
///
/// The callback is queried several times per frame and per contact, from
/// inside the frame. It must not block, perform I/O or allocate, and it
/// must be deterministic in its arguments and its own state for the
/// simulation to stay reproducible. It is not copied or serialised with
/// the models.
///
/// # Velocities
///
/// `velocity` must be the surface velocity at the queried point, including
/// rotation; `angular_velocity` reaches only the friction solve. A rotating
/// deck therefore returns its translation plus `ω × r` for each point, as
/// `FGLGear` takes the contact's velocity and discards its angular velocity.
///
/// # Contact point
///
/// [`Contact::point`] is used for its geodetic altitude (terrain
/// elevation, `SetAltitudeAGL`) and its radius (`metrics/terrain-radius`),
/// so a callback should give it the planet's ellipse with
/// [`Location::set_ellipse`] and then call [`Location::cache_derived`],
/// so that those reads do not recompute the geodetic conversion. JSBSim's callers set an ellipse on the
/// location they pass in at some call sites and not at others, which a
/// callback that only moves the location inherits; a returned contact has
/// exactly the ellipse the callback gave it.
///
/// The location queried may have no ellipse (gear positions come from
/// `LocalToLocation`), so a callback should read only its ECEF
/// coordinates, or copy it and set the ellipse first as
/// [`DefaultGroundCallback`] does.
pub trait GroundCallback: Send {
    /// `GetAGLevel(t, location, contact, normal, v, w)`: the terrain under
    /// `location` at simulation time `time`, s.
    fn ag_level(&self, time: f64, location: &Location) -> Contact;

    /// `SetTerrainElevation(h)`: JSBSim's `<elevation>` and
    /// `position/terrain-elevation-asl-ft`. Ignored unless the callback
    /// lets JSBSim move its terrain, as in JSBSim.
    fn set_terrain_elevation(&mut self, _elevation: f64) {}

    /// `SetEllipse(semimajor, semiminor)`: the planet's axes, ft, when a
    /// `<planet>` is loaded. Ignored unless the callback follows the
    /// planet's shape, as in JSBSim.
    fn set_ellipse(&mut self, _semimajor: f64, _semiminor: f64) {}
}

/// `FGDefaultGroundCallback`: terrain at a uniform height above the
/// ellipsoid, at rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DefaultGroundCallback {
    a: f64,
    b: f64,
    terrain_elevation: f64,
}

impl DefaultGroundCallback {
    /// `FGDefaultGroundCallback(semiMajor, semiMinor)`, terrain at sea
    /// level.
    #[must_use]
    pub fn new(semimajor: f64, semiminor: f64) -> Self {
        Self {
            a: semimajor,
            b: semiminor,
            terrain_elevation: 0.0,
        }
    }

    /// The terrain's height above the ellipsoid, ft.
    #[must_use]
    pub fn terrain_elevation(&self) -> f64 {
        self.terrain_elevation
    }
}

impl GroundCallback for DefaultGroundCallback {
    fn ag_level(&self, _time: f64, location: &Location) -> Contact {
        let mut l = *location;
        l.set_ellipse(self.a, self.b);
        l.cache_derived();
        let latitude = l.geod_latitude_rad();
        let (sin_lat, cos_lat) = libm::sincos(latitude);
        let longitude = l.longitude();
        let (sin_lon, cos_lon) = libm::sincos(longitude);
        let normal = ColumnVector3::new(cos_lat * cos_lon, cos_lat * sin_lon, sin_lat);
        let mut point = Location::default();
        point.set_ellipse(self.a, self.b);
        point.set_position_geodetic(longitude, latitude, self.terrain_elevation);
        Contact {
            agl: l.geod_altitude() - self.terrain_elevation,
            point,
            normal,
            velocity: ColumnVector3::zero(),
            angular_velocity: ColumnVector3::zero(),
        }
    }

    fn set_terrain_elevation(&mut self, elevation: f64) {
        self.terrain_elevation = elevation;
    }

    fn set_ellipse(&mut self, semimajor: f64, semiminor: f64) {
        self.a = semimajor;
        self.b = semiminor;
    }
}

/// `FGInertial::GroundCallback`: the default callback held without
/// indirection, or one installed in its place.
pub(super) enum Slot {
    /// The callback `FGInertial`'s constructor makes.
    Default(DefaultGroundCallback),
    /// One installed with `SetGroundCallback`.
    Custom(Box<dyn GroundCallback>),
}

impl Slot {
    pub(super) fn get(&self) -> &dyn GroundCallback {
        match self {
            Self::Default(d) => d,
            Self::Custom(c) => c.as_ref(),
        }
    }

    pub(super) fn get_mut(&mut self) -> &mut dyn GroundCallback {
        match self {
            Self::Default(d) => d,
            Self::Custom(c) => c.as_mut(),
        }
    }
}

impl std::fmt::Debug for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default(d) => d.fmt(f),
            Self::Custom(_) => f.write_str("Custom"),
        }
    }
}
