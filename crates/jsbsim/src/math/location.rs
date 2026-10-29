// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/math/FGLocation.{h,cpp}.
//
//! Earth-centred, Earth-fixed position with cached longitude,
//! latitude, radius, geodetic latitude and altitude, and local-frame
//! transformations, plus geodesic distance and heading on the ellipse
//! (GeographicLib's inverse problem, as JSBSim uses it).

mod snapshot;

use super::geodesic::Geodesic;
use super::{Axis, ColumnVector3, E_X, E_Y, E_Z, Matrix33};
use crate::units::{DEGTORAD, RADTODEG, sign};

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
///
/// The derived coordinates and matrices are a pure function of the ECEF
/// coordinates and the ellipse. [`cache_derived`](Self::cache_derived)
/// stores them until the next mutation, as JSBSim's `mCacheValid` does; a
/// read without a cached value computes them on the spot, with the same
/// operations.
#[derive(Debug, Clone, Copy)]
pub struct Location {
    ec_loc: ColumnVector3,
    ellipse: Option<Ellipse>,
    derived: Option<Derived>,
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
            derived: None,
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
        self.derived = None;
    }

    /// Computes and stores the derived values unless already stored;
    /// owners of a long-lived location call it after each change so that
    /// later reads through `&self` find them.
    pub fn cache_derived(&mut self) {
        if self.derived.is_none() {
            self.derived = Some(compute_derived(self.ec_loc, self.ellipse));
        }
    }

    /// The ECEF coordinates.
    #[must_use]
    pub fn ecef(&self) -> ColumnVector3 {
        self.ec_loc
    }

    /// Component `idx` of the ECEF coordinates.
    #[must_use]
    pub fn entry(&self, idx: Axis) -> f64 {
        self.ec_loc[idx]
    }

    /// Sets component `idx` of the ECEF coordinates.
    pub fn set_entry(&mut self, idx: Axis, value: f64) {
        self.ec_loc[idx] = value;
        self.invalidate();
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
        let (sin_lon, cos_lon) = libm::sincos(longitude);
        self.ec_loc[E_X] = rtmp * cos_lon;
        self.ec_loc[E_Y] = rtmp * sin_lon;
        self.invalidate();
    }

    /// `SetLatitude`.
    pub fn set_latitude(&mut self, latitude: f64) {
        let mut r = self.ec_loc.magnitude();
        if r == 0.0 {
            self.ec_loc[E_X] = 1.0;
            r = 1.0;
        }
        let rtmp = self.ec_loc.magnitude2(E_X, E_Y);
        let (sin_lat, cos_lat) = libm::sincos(latitude);
        if rtmp != 0.0 {
            let fac = r / rtmp * cos_lat;
            self.ec_loc[E_X] *= fac;
            self.ec_loc[E_Y] *= fac;
        } else {
            self.ec_loc[E_X] = r * cos_lat;
            self.ec_loc[E_Y] = 0.0;
        }
        self.ec_loc[E_Z] = r * sin_lat;
        self.invalidate();
    }

    /// `SetRadius`.
    pub fn set_radius(&mut self, radius: f64) {
        let rold = self.ec_loc.magnitude();
        if rold == 0.0 {
            self.ec_loc[E_X] = radius;
        } else {
            self.ec_loc *= radius / rold;
        }
        self.invalidate();
    }

    /// `SetPosition(lon, lat, radius)`: geocentric coordinates.
    pub fn set_position(&mut self, lon: f64, lat: f64, radius: f64) {
        let (sin_lat, cos_lat) = libm::sincos(lat);
        let (sin_lon, cos_lon) = libm::sincos(lon);
        self.ec_loc = ColumnVector3::new(
            radius * cos_lat * cos_lon,
            radius * cos_lat * sin_lon,
            radius * sin_lat,
        );
        self.invalidate();
    }

    /// `SetPositionGeodetic(lon, lat, height)`; requires an ellipse, as
    /// JSBSim asserts, and does nothing without one.
    pub fn set_position_geodetic(&mut self, lon: f64, lat: f64, height: f64) {
        let Some(e) = self.ellipse else {
            tracing::error!("FGLocation::SetPositionGeodetic called before SetEllipse");
            return;
        };
        let (slat, clat) = libm::sincos(lat);
        let (slon, clon) = libm::sincos(lon);
        let rn = e.a / (1.0 - e.e2 * slat * slat).sqrt();
        self.ec_loc[E_X] = (rn + height) * clat * clon;
        self.ec_loc[E_Y] = (rn + height) * clat * slon;
        self.ec_loc[E_Z] = ((1.0 - e.e2) * rn + height) * slat;
        self.invalidate();
    }

    /// `SetEllipse(semimajor, semiminor)`.
    pub fn set_ellipse(&mut self, semimajor: f64, semiminor: f64) {
        self.ellipse = Some(Ellipse::new(semimajor, semiminor));
        self.invalidate();
    }

    fn read<R>(&self, f: impl FnOnce(&Derived) -> R) -> R {
        match &self.derived {
            Some(d) => f(d),
            None => {
                super::uncached_derived_read("Location");
                f(&compute_derived(self.ec_loc, self.ellipse))
            }
        }
    }

    /// `GetLongitude()`, in (-pi, pi].
    #[must_use]
    pub fn longitude(&self) -> f64 {
        self.read(|d| d.lon)
    }

    /// `GetLongitudeDeg()`.
    #[must_use]
    pub fn longitude_deg(&self) -> f64 {
        RADTODEG * self.read(|d| d.lon)
    }

    /// `GetSinLongitude()`.
    #[must_use]
    pub fn sin_longitude(&self) -> f64 {
        -self.read(|d| d.tec2l[(Axis::Y, Axis::X)])
    }

    /// `GetCosLongitude()`.
    #[must_use]
    pub fn cos_longitude(&self) -> f64 {
        self.read(|d| d.tec2l[(Axis::Y, Axis::Y)])
    }

    /// `GetLatitude()`: geocentric latitude.
    #[must_use]
    pub fn latitude(&self) -> f64 {
        self.read(|d| d.lat)
    }

    /// `GetLatitudeDeg()`.
    #[must_use]
    pub fn latitude_deg(&self) -> f64 {
        RADTODEG * self.read(|d| d.lat)
    }

    /// `GetGeodLatitudeRad()`; 0 without an ellipse.
    #[must_use]
    pub fn geod_latitude_rad(&self) -> f64 {
        self.read(|d| d.geod_lat)
    }

    /// `GetGeodLatitudeDeg()`.
    #[must_use]
    pub fn geod_latitude_deg(&self) -> f64 {
        RADTODEG * self.read(|d| d.geod_lat)
    }

    /// `GetGeodAltitude()`; 0 without an ellipse.
    #[must_use]
    pub fn geod_altitude(&self) -> f64 {
        self.read(|d| d.geod_alt)
    }

    /// `GetSeaLevelRadius()`. Without an ellipse JSBSim's defaults
    /// (`a = ec = 1`, `e2 = 0`) apply, giving 1.
    #[must_use]
    pub fn sea_level_radius(&self) -> f64 {
        let e = self.ellipse.unwrap_or(Ellipse::new(1.0, 1.0));
        let cos_lat = libm::cos(self.read(|d| d.lat));
        e.a * e.ec / (1.0 - e.e2 * cos_lat * cos_lat).sqrt()
    }

    /// `GetRadius()`.
    #[must_use]
    pub fn radius(&self) -> f64 {
        self.read(|d| d.radius)
    }

    /// `GetTl2ec()`: local (NED) to ECEF.
    #[must_use]
    pub fn tl2ec(&self) -> Matrix33 {
        self.read(|d| d.tl2ec)
    }

    /// `GetTec2l()`: ECEF to local (NED).
    #[must_use]
    pub fn tec2l(&self) -> Matrix33 {
        self.read(|d| d.tec2l)
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
        self.ec_loc += l.ec_loc;
        self.invalidate();
    }

    /// `operator-=`.
    pub fn sub_assign(&mut self, l: &Self) {
        self.ec_loc -= l.ec_loc;
        self.invalidate();
    }

    /// `operator*=`.
    pub fn scale_assign(&mut self, scalar: f64) {
        self.ec_loc *= scalar;
        self.invalidate();
    }

    /// `operator/=`: scaling by `1/scalar`.
    pub fn div_assign(&mut self, scalar: f64) {
        self.scale_assign(1.0 / scalar);
    }

    /// The geodesic on this location's ellipse. JSBSim asserts that an
    /// ellipse is set; without one its defaults (`a = ec = 1`) describe
    /// the unit sphere, which is used here too.
    fn geodesic(&self) -> Geodesic {
        let e = self.ellipse.unwrap_or(Ellipse::new(1.0, 1.0));
        Geodesic::new(e.a, 1.0 - e.ec)
    }

    /// `GetDistanceTo(lon, lat)`: geodesic distance, ft, to the target
    /// longitude and geodetic latitude (radians).
    #[must_use]
    pub fn distance_to(&self, target_longitude: f64, target_latitude: f64) -> f64 {
        let (geod_lat, lon) = self.read(|d| (d.geod_lat, d.lon));
        self.geodesic().inverse_distance(
            geod_lat * RADTODEG,
            lon * RADTODEG,
            target_latitude * RADTODEG,
            target_longitude * RADTODEG,
        )
    }

    /// `GetHeadingTo(lon, lat)`: initial geodesic azimuth, rad, in
    /// [-pi, pi], to the target longitude and geodetic latitude (radians).
    #[must_use]
    pub fn heading_to(&self, target_longitude: f64, target_latitude: f64) -> f64 {
        let (geod_lat, lon) = self.read(|d| (d.geod_lat, d.lon));
        let (azi1, _azi2) = self.geodesic().inverse_azimuths(
            geod_lat * RADTODEG,
            lon * RADTODEG,
            target_latitude * RADTODEG,
            target_longitude * RADTODEG,
        );
        azi1 * DEGTORAD
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
