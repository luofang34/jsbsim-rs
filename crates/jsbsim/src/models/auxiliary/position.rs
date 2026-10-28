// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGAuxiliary.cpp (SetInitialState,
// Get*RelativePosition, GetNEUPositionFromStart).
//
//! Position relative to the initial conditions.

use super::Auxiliary;
use crate::math::{ColumnVector3, E_Z, Location};
use crate::units::FTTOM;

impl Auxiliary {
    /// `SetInitialState`: the NEU frame's origin is the initial position
    /// brought down to the ellipse. JSBSim's initial position always
    /// carries the planet's ellipse, which `semimajor` and `semiminor`
    /// give.
    pub fn set_initial_state(&mut self, ic_position: &Location, semimajor: f64, semiminor: f64) {
        let (lon, lat) = self.set_ic_reference(ic_position, semimajor, semiminor);
        let mut start = ic_position.clone();
        start.set_ellipse(semimajor, semiminor);
        start.set_position_geodetic(lon, lat, 0.0);
        self.start.neu_location = start;
    }

    /// Records the initial-condition longitude and geodetic latitude the
    /// distances from start are measured to, as JSBSim reads them from
    /// `FGInitialCondition`, and returns them. The latitude is taken on
    /// the planet's ellipse, which JSBSim's initial position carries.
    pub fn set_ic_reference(
        &mut self,
        ic_position: &Location,
        semimajor: f64,
        semiminor: f64,
    ) -> (f64, f64) {
        let mut p = ic_position.clone();
        p.set_ellipse(semimajor, semiminor);
        let (lon, lat) = (p.longitude(), p.geod_latitude_rad());
        self.start.ic_longitude = lon;
        self.start.ic_geod_latitude = lat;
        (lon, lat)
    }

    /// `GetLongitudeRelativePosition`: distance, m, along the current
    /// latitude to the initial longitude.
    #[must_use]
    pub fn longitude_relative_position(&self) -> f64 {
        let l = &self.input.location;
        l.distance_to(self.start.ic_longitude, l.geod_latitude_rad()) * FTTOM
    }

    /// `GetLatitudeRelativePosition`: distance, m, along the current
    /// meridian to the initial latitude.
    #[must_use]
    pub fn latitude_relative_position(&self) -> f64 {
        let l = &self.input.location;
        l.distance_to(l.longitude(), self.start.ic_geod_latitude) * FTTOM
    }

    /// `GetDistanceRelativePosition`: geodesic distance, m, to the initial
    /// position.
    #[must_use]
    pub fn distance_relative_position(&self) -> f64 {
        self.input
            .location
            .distance_to(self.start.ic_longitude, self.start.ic_geod_latitude)
            * FTTOM
    }

    /// `GetNEUPositionFromStart`: position, ft, in the NEU frame fixed at
    /// [`Start::neu_location`](super::Start::neu_location). JSBSim caches
    /// it until the next run; it is computed on each call here.
    #[must_use]
    pub fn neu_position_from_start(&self) -> ColumnVector3 {
        let mut neu = self
            .start
            .neu_location
            .location_to_local(self.input.location.ecef());
        neu[E_Z] *= -1.0;
        neu
    }
}
