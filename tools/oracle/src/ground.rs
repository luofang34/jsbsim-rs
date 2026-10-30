// SPDX-License-Identifier: LGPL-2.1-or-later
//! The terrains of tools/oracle/cpp/ground_callbacks.cpp, operation for
//! operation, which a script case names in its manifest's
//! `ground_callback`. Each is a plane through the ECEF point (a, 0, 0),
//! where the planet's surface crosses latitude 0 and longitude 0 (local
//! north is +z, east +y, up +x); see that file for what each does.

use jsbsim::Fdm;
use jsbsim::math::{ColumnVector3, Location, dot_product};
use jsbsim::{Contact, GroundCallback};

use crate::OracleError;

/// Which terrain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Tilted down towards the north by atan(5/12), at rest.
    Slope,
    /// Level, moving at a constant ECEF velocity.
    DeckMoving,
    /// Level, spinning about its normal.
    DeckRotating,
    /// Level, rising and falling with time.
    DeckHeaving,
}

/// Every terrain with its manifest name, in the order of the C++ file.
pub const KINDS: &[(&str, Kind)] = &[
    ("slope", Kind::Slope),
    ("deck-moving", Kind::DeckMoving),
    ("deck-rotating", Kind::DeckRotating),
    ("deck-heaving", Kind::DeckHeaving),
];

impl Kind {
    /// The terrain named `name`.
    pub fn parse(name: &str) -> Result<Self, OracleError> {
        KINDS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|&(_, kind)| kind)
            .ok_or_else(|| OracleError::Provenance(format!("no ground callback named {name}")))
    }
}

/// `OracleGround`.
#[derive(Debug, Clone, Copy)]
pub struct OracleGround {
    kind: Kind,
    a: f64,
    b: f64,
}

impl OracleGround {
    /// `kind` on a planet of semi-axes `a` and `b`, ft.
    #[must_use]
    pub fn new(kind: Kind, a: f64, b: f64) -> Self {
        Self { kind, a, b }
    }
}

impl GroundCallback for OracleGround {
    fn ag_level(&self, t: f64, location: &Location) -> Contact {
        let mut origin = ColumnVector3::new(self.a, 0.0, 0.0);
        let mut n = ColumnVector3::new(1.0, 0.0, 0.0);
        let mut vel = ColumnVector3::zero();
        let mut omega = ColumnVector3::zero();
        match self.kind {
            Kind::Slope => n = ColumnVector3::new(12.0 / 13.0, 0.0, 5.0 / 13.0),
            Kind::DeckMoving => {
                vel = ColumnVector3::new(0.5, 20.0, 0.0);
                origin += vel * t;
            }
            Kind::DeckRotating => omega = ColumnVector3::new(0.3, 0.0, 0.0),
            Kind::DeckHeaving => {
                let phase = (t % 4.0) / 4.0;
                let s = if phase < 0.5 {
                    2.0 * phase
                } else {
                    2.0 - 2.0 * phase
                };
                let ds = if phase < 0.5 { 0.5 } else { -0.5 };
                let height = s * s * (3.0 - 2.0 * s);
                vel = n * (6.0 * s * (1.0 - s) * ds);
                origin += n * height;
            }
        }
        let r = location.ecef();
        let agl = dot_product(r - origin, n);
        let point = r - agl * n;
        if self.kind == Kind::DeckRotating {
            let centre = ColumnVector3::new(self.a, 0.0, -30.0);
            vel = omega.cross(point - centre);
        }
        let mut contact = Location::from_ecef(point);
        contact.set_ellipse(self.a, self.b);
        contact.cache_derived();
        Contact::new(agl, contact, n).with_motion(vel, omega)
    }
}

/// Installs the terrain named `name` on `fdm`, with its planet's axes.
pub fn install(fdm: &mut Fdm, name: &str) -> Result<(), OracleError> {
    let kind = Kind::parse(name)?;
    let (a, b) = (fdm.models.inertial.a, fdm.models.inertial.b);
    fdm.set_ground_callback(Box::new(OracleGround::new(kind, a, b)));
    Ok(())
}
