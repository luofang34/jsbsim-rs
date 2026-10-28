// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/math/FGLocation.{h,cpp}.
//
//! Earth-centred, Earth-fixed position with lazily derived longitude,
//! latitude, radius, geodetic latitude and altitude, and local-frame
//! transformations.
//!
//! JSBSim's geodesic distance and heading (`GetDistanceTo`,
//! `GetHeadingTo`) use GeographicLib and are not part of this type yet.

use std::cell::OnceCell;

use super::{ColumnVector3, E_X, E_Y, E_Z, Matrix33};
use crate::units::{RADTODEG, sign};

#[derive(Debug, Clone, Copy, PartialEq)]
struct Derived {
    lon: f64,
    lat: f64,
    radius: f64,
    geod_lat: f64,
    geod_alt: f64,
    tl2ec: Matrix33,
    tec2l: Matrix33,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Ellipse {
    a: f64,
    e2: f64,
    c: f64,
    ec: f64,
    ec2: f64,
}

impl Ellipse {
    fn new(semimajor: f64, semiminor: f64) -> Self {
        let a = semimajor;
        let ec = semiminor / a;
        let ec2 = ec * ec;
        let e2 = 1.0 - ec2;
        Self {
            a,
            e2,
            c: a * e2,
            ec,
            ec2,
        }
    }
}

/// `FGLocation`.
#[derive(Debug, Clone)]
pub struct Location {
    ec_loc: ColumnVector3,
    ellipse: Option<Ellipse>,
    derived: OnceCell<Derived>,
}

impl PartialEq for Location {
    /// Equal cartesian coordinates (`operator==`).
    fn eq(&self, other: &Self) -> bool {
        self.ec_loc == other.ec_loc
    }
}

impl Default for Location {
    /// `FGLocation()`: the unit vector along x.
    fn default() -> Self {
        Self::from_ecef(ColumnVector3::new(1.0, 0.0, 0.0))
    }
}

impl Location {
    /// From ECEF coordinates, without an ellipse.
    #[must_use]
    pub fn from_ecef(v: ColumnVector3) -> Self {
        Self {
            ec_loc: v,
            ellipse: None,
            derived: OnceCell::new(),
        }
    }

    /// From geocentric longitude and latitude (radians) and radius.
    #[must_use]
    pub fn from_spherical(lon: f64, lat: f64, radius: f64) -> Self {
        let mut l = Self::from_ecef(ColumnVector3::zero());
        l.set_position(lon, lat, radius);
        l
    }

    fn invalidate(&mut self) {
        self.derived = OnceCell::new();
    }

    /// The ECEF coordinates.
    #[must_use]
    pub fn ecef(&self) -> ColumnVector3 {
        self.ec_loc
    }

    /// Component `idx` (1-based) of the ECEF coordinates.
    #[must_use]
    pub fn entry(&self, idx: usize) -> f64 {
        self.ec_loc[idx]
    }

    /// Sets component `idx` (1-based) of the ECEF coordinates.
    pub fn set_entry(&mut self, idx: usize, value: f64) {
        self.invalidate();
        self.ec_loc[idx] = value;
    }

    /// `operator=(FGColumnVector3)`: new coordinates, ellipse kept.
    pub fn assign_ecef(&mut self, v: ColumnVector3) {
        self.ec_loc = v;
        self.invalidate();
    }

    /// `SetLongitude`.
    pub fn set_longitude(&mut self, longitude: f64) {
        let mut rtmp = self.ec_loc.magnitude2(E_X, E_Y);
        if 0.0 == self.ec_loc.magnitude() {
            rtmp = 1.0;
        }
        if rtmp == 0.0 {
            return;
        }
        self.invalidate();
        self.ec_loc[E_X] = rtmp * libm::cos(longitude);
        self.ec_loc[E_Y] = rtmp * libm::sin(longitude);
    }

    /// `SetLatitude`.
    pub fn set_latitude(&mut self, latitude: f64) {
        self.invalidate();
        let mut r = self.ec_loc.magnitude();
        if r == 0.0 {
            self.ec_loc[E_X] = 1.0;
            r = 1.0;
        }
        let rtmp = self.ec_loc.magnitude2(E_X, E_Y);
        if rtmp != 0.0 {
            let fac = r / rtmp * libm::cos(latitude);
            self.ec_loc[E_X] *= fac;
            self.ec_loc[E_Y] *= fac;
        } else {
            self.ec_loc[E_X] = r * libm::cos(latitude);
            self.ec_loc[E_Y] = 0.0;
        }
        self.ec_loc[E_Z] = r * libm::sin(latitude);
    }

    /// `SetRadius`.
    pub fn set_radius(&mut self, radius: f64) {
        self.invalidate();
        let rold = self.ec_loc.magnitude();
        if rold == 0.0 {
            self.ec_loc[E_X] = radius;
        } else {
            self.ec_loc *= radius / rold;
        }
    }

    /// `SetPosition(lon, lat, radius)`: geocentric coordinates.
    pub fn set_position(&mut self, lon: f64, lat: f64, radius: f64) {
        self.invalidate();
        let (sin_lat, cos_lat) = (libm::sin(lat), libm::cos(lat));
        let (sin_lon, cos_lon) = (libm::sin(lon), libm::cos(lon));
        self.ec_loc = ColumnVector3::new(
            radius * cos_lat * cos_lon,
            radius * cos_lat * sin_lon,
            radius * sin_lat,
        );
    }

    /// `SetPositionGeodetic(lon, lat, height)`; requires an ellipse, as
    /// JSBSim asserts, and does nothing without one.
    pub fn set_position_geodetic(&mut self, lon: f64, lat: f64, height: f64) {
        let Some(e) = self.ellipse else {
            tracing::error!("FGLocation::SetPositionGeodetic called before SetEllipse");
            return;
        };
        self.invalidate();
        let (slat, clat) = (libm::sin(lat), libm::cos(lat));
        let rn = e.a / (1.0 - e.e2 * slat * slat).sqrt();
        self.ec_loc[E_X] = (rn + height) * clat * libm::cos(lon);
        self.ec_loc[E_Y] = (rn + height) * clat * libm::sin(lon);
        self.ec_loc[E_Z] = ((1.0 - e.e2) * rn + height) * slat;
    }

    /// `SetEllipse(semimajor, semiminor)`.
    pub fn set_ellipse(&mut self, semimajor: f64, semiminor: f64) {
        self.invalidate();
        self.ellipse = Some(Ellipse::new(semimajor, semiminor));
    }

    fn derived(&self) -> &Derived {
        self.derived
            .get_or_init(|| compute_derived(self.ec_loc, self.ellipse))
    }

    /// `GetLongitude()`, in (-pi, pi].
    #[must_use]
    pub fn longitude(&self) -> f64 {
        self.derived().lon
    }

    /// `GetLongitudeDeg()`.
    #[must_use]
    pub fn longitude_deg(&self) -> f64 {
        RADTODEG * self.derived().lon
    }

    /// `GetSinLongitude()`.
    #[must_use]
    pub fn sin_longitude(&self) -> f64 {
        -self.derived().tec2l[(2, 1)]
    }

    /// `GetCosLongitude()`.
    #[must_use]
    pub fn cos_longitude(&self) -> f64 {
        self.derived().tec2l[(2, 2)]
    }

    /// `GetLatitude()`: geocentric latitude.
    #[must_use]
    pub fn latitude(&self) -> f64 {
        self.derived().lat
    }

    /// `GetLatitudeDeg()`.
    #[must_use]
    pub fn latitude_deg(&self) -> f64 {
        RADTODEG * self.derived().lat
    }

    /// `GetGeodLatitudeRad()`; 0 without an ellipse.
    #[must_use]
    pub fn geod_latitude_rad(&self) -> f64 {
        self.derived().geod_lat
    }

    /// `GetGeodLatitudeDeg()`.
    #[must_use]
    pub fn geod_latitude_deg(&self) -> f64 {
        RADTODEG * self.derived().geod_lat
    }

    /// `GetGeodAltitude()`; 0 without an ellipse.
    #[must_use]
    pub fn geod_altitude(&self) -> f64 {
        self.derived().geod_alt
    }

    /// `GetSeaLevelRadius()`. Without an ellipse JSBSim's defaults
    /// (`a = ec = 1`, `e2 = 0`) apply, giving 1.
    #[must_use]
    pub fn sea_level_radius(&self) -> f64 {
        let e = self.ellipse.unwrap_or(Ellipse::new(1.0, 1.0));
        let cos_lat = libm::cos(self.derived().lat);
        e.a * e.ec / (1.0 - e.e2 * cos_lat * cos_lat).sqrt()
    }

    /// `GetRadius()`.
    #[must_use]
    pub fn radius(&self) -> f64 {
        self.derived().radius
    }

    /// `GetTl2ec()`: local (NED) to ECEF.
    #[must_use]
    pub fn tl2ec(&self) -> Matrix33 {
        self.derived().tl2ec
    }

    /// `GetTec2l()`: ECEF to local (NED).
    #[must_use]
    pub fn tec2l(&self) -> Matrix33 {
        self.derived().tec2l
    }

    /// `LocalToLocation(lvec)`. Like JSBSim, the result has no ellipse.
    #[must_use]
    pub fn local_to_location(&self, lvec: ColumnVector3) -> Self {
        Self::from_ecef(self.tl2ec() * lvec + self.ec_loc)
    }

    /// `LocationToLocal(ecvec)`.
    #[must_use]
    pub fn location_to_local(&self, ecvec: ColumnVector3) -> ColumnVector3 {
        self.tec2l() * (ecvec - self.ec_loc)
    }

    /// Builds the result of a binary operator: like JSBSim, the ellipse is
    /// set again from `a` and `ec * a`.
    fn with_ellipse_of(&self, v: ColumnVector3) -> Self {
        let mut result = Self::from_ecef(v);
        if let Some(e) = self.ellipse {
            result.set_ellipse(e.a, e.ec * e.a);
        }
        result
    }

    /// `operator+`.
    #[must_use]
    pub fn add(&self, l: &Self) -> Self {
        self.with_ellipse_of(self.ec_loc + l.ec_loc)
    }

    /// `operator-`.
    #[must_use]
    pub fn sub(&self, l: &Self) -> Self {
        self.with_ellipse_of(self.ec_loc - l.ec_loc)
    }

    /// `operator*(scalar)`.
    #[must_use]
    pub fn scale(&self, scalar: f64) -> Self {
        self.with_ellipse_of(scalar * self.ec_loc)
    }

    /// `operator+=`.
    pub fn add_assign(&mut self, l: &Self) {
        self.invalidate();
        self.ec_loc += l.ec_loc;
    }

    /// `operator-=`.
    pub fn sub_assign(&mut self, l: &Self) {
        self.invalidate();
        self.ec_loc -= l.ec_loc;
    }

    /// `operator*=`.
    pub fn scale_assign(&mut self, scalar: f64) {
        self.invalidate();
        self.ec_loc *= scalar;
    }

    /// `operator/=`: scaling by `1/scalar`.
    pub fn div_assign(&mut self, scalar: f64) {
        self.scale_assign(1.0 / scalar);
    }
}

fn compute_derived(ec: ColumnVector3, ellipse: Option<Ellipse>) -> Derived {
    let radius = ec.magnitude();
    let rxy = ec.magnitude2(E_X, E_Y);
    let (sin_lon, cos_lon, lon) = if rxy == 0.0 {
        (0.0, 1.0, 0.0)
    } else {
        (ec[E_Y] / rxy, ec[E_X] / rxy, libm::atan2(ec[E_Y], ec[E_X]))
    };
    let (mut geod_lat, mut geod_alt) = (0.0, 0.0);
    let (lat, sin_lat, cos_lat);
    if radius == 0.0 {
        (lat, sin_lat, cos_lat) = (0.0, 0.0, 1.0);
        if let Some(e) = ellipse {
            geod_alt = -e.a;
        }
    } else {
        lat = libm::atan2(ec[E_Z], rxy);
        if let Some(e) = ellipse {
            let g = geodetic(ec[E_Z], rxy, &e);
            (geod_lat, geod_alt, sin_lat, cos_lat) = (g.lat, g.alt, g.sin_lat, g.cos_lat);
        } else {
            (sin_lat, cos_lat) = (ec[E_Z] / radius, rxy / radius);
        }
    }
    let tec2l = Matrix33::new(
        -cos_lon * sin_lat,
        -sin_lon * sin_lat,
        cos_lat,
        -sin_lon,
        cos_lon,
        0.0,
        -cos_lon * cos_lat,
        -sin_lon * cos_lat,
        -sin_lat,
    );
    Derived {
        lon,
        lat,
        radius,
        geod_lat,
        geod_alt,
        tl2ec: tec2l.transposed(),
        tec2l,
    }
}

struct Geodetic {
    lat: f64,
    alt: f64,
    sin_lat: f64,
    cos_lat: f64,
}

/// Geodetic latitude and altitude by JSBSim's closed-form iteration.
fn geodetic(z: f64, rxy: f64, e: &Ellipse) -> Geodetic {
    let s0 = z.abs();
    let zc = e.ec * s0;
    let c0 = e.ec * rxy;
    let c02 = c0 * c0;
    let s02 = s0 * s0;
    let a02 = c02 + s02;
    let a0 = a02.sqrt();
    let a03 = a02 * a0;
    let mut s1 = zc * a03 + e.c * s02 * s0;
    let c1 = rxy * a03 - e.c * c02 * c0;
    let cs0c0 = e.c * c0 * s0;
    let b0 = 1.5 * cs0c0 * ((rxy * s0 - zc * c0) * a0 - cs0c0);
    s1 = s1 * a03 - b0 * s0;
    let cc = e.ec * (c1 * a03 - b0 * c0);
    let lat = sign(z) * libm::atan(s1 / cc);
    let s12 = s1 * s1;
    let cc2 = cc * cc;
    let norm = (s12 + cc2).sqrt();
    Geodetic {
        lat,
        alt: (rxy * cc + s0 * s1 - e.a * (e.ec2 * s12 + cc2).sqrt()) / norm,
        sin_lat: sign(z) * s1 / norm,
        cos_lat: cc / norm,
    }
}
