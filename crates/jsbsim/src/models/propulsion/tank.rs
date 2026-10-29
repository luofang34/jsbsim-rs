// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGTank.{h,cpp}.
//
//! A fuel or oxidizer tank (`FGTank`): contents, temperature, standpipe,
//! feed priority, and the location and inertia its contents contribute
//! to the mass balance.

mod bind;
mod load;

pub use bind::bind;

use crate::math::ColumnVector3;
use crate::units::LBTOSLUG;

/// `FGTank::TankType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TankType {
    /// `ttUNKNOWN`: any `type` but the two below, which the propulsion
    /// loader refuses.
    Unknown,
    /// `ttFUEL`.
    Fuel,
    /// `ttOXIDIZER`.
    Oxidizer,
}

/// `FGTank::GrainType`: the shape of a solid propellant grain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrainType {
    /// `gtUNKNOWN`: no `<grain_config>`, a liquid.
    Unknown,
    /// `gtCYLINDRICAL`: a grain with a bore.
    Cylindrical,
    /// `gtENDBURNING`: a solid cylinder burning from one end.
    EndBurning,
}

/// A solid propellant grain's geometry, inches and cubic inches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grain {
    /// Shape.
    pub kind: GrainType,
    /// Grain length.
    pub length: f64,
    /// Bore radius.
    pub inner_radius: f64,
    /// Propellant volume.
    pub volume: f64,
}

/// What `ResetToIC` restores.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Initial {
    /// Contents, lbs. JSBSim leaves `InitialContents` uninitialized when
    /// the tank has no `<contents>`; the port starts it at 0, as the
    /// contents themselves start.
    pub contents: f64,
    /// Temperature, °C, or -9999 for none.
    pub temperature: f64,
    /// Standpipe, lbs.
    pub standpipe: f64,
    /// Feed priority.
    pub priority: i32,
}

/// `FGTank`.
#[derive(Debug, Clone, PartialEq)]
pub struct Tank {
    /// Fuel or oxidizer.
    pub tank_type: TankType,
    /// Solid propellant geometry.
    pub grain: Grain,
    /// Index among the propulsion's tanks.
    pub number: usize,
    /// `name` attribute.
    pub name: String,
    /// Location of the full tank's CG, structural frame, in.
    pub xyz: ColumnVector3,
    /// Location of the empty tank's CG, in.
    pub xyz_drain: ColumnVector3,
    /// Capacity, lbs.
    pub capacity: f64,
    /// Unusable volume, gal.
    pub unusable_volume: f64,
    /// Effective radius, in.
    pub radius: f64,
    /// Density, lbs/gal (slugs/in^3 for a solid grain).
    pub density: f64,
    /// Local moments of inertia about the tank CG, slug·ft^2.
    pub ixx: f64,
    /// See `ixx`.
    pub iyy: f64,
    /// See `ixx`.
    pub izz: f64,
    /// Liquid inertia relative to a solid of the same shape.
    pub inertia_factor: f64,
    /// Fill level, 0 to 100.
    pub pct_full: f64,
    /// Contents, lbs.
    pub contents: f64,
    /// Area of one side for heat exchange, ft^2.
    pub area: f64,
    /// Temperature, °C, or -9999 when the tank has none.
    pub temperature: f64,
    /// Contents that cannot be dumped, lbs.
    pub standpipe: f64,
    /// External flow into the tank (negative out of it), lbs/s.
    pub external_flow: f64,
    /// Whether the priority is non-zero.
    pub selected: bool,
    /// Feed priority, 1 the highest.
    pub priority: i32,
    /// What `ResetToIC` restores.
    pub initial: Initial,
}

impl Tank {
    /// `GetUnusable`: unusable contents, lbs.
    #[must_use]
    pub fn unusable(&self) -> f64 {
        self.unusable_volume * self.density
    }

    /// `SetPriority`, which also selects the tank when `p` is positive.
    pub fn set_priority(&mut self, p: i32) {
        self.priority = p;
        self.selected = p > 0;
    }

    /// `GetXYZ`: the contents' CG between drain and full locations.
    #[must_use]
    pub fn xyz(&self) -> ColumnVector3 {
        self.xyz_drain + (self.contents / self.capacity) * (self.xyz - self.xyz_drain)
    }

    /// `ResetToIC`.
    pub fn reset_to_ic(&mut self) {
        self.temperature = self.initial.temperature;
        self.standpipe = self.initial.standpipe;
        self.set_contents(self.initial.contents);
        self.pct_full = 100.0 * self.contents / self.capacity;
        self.set_priority(self.initial.priority);
        self.calculate_inertias();
    }

    /// `Drain`: removes `used` lbs down to the unusable contents; the
    /// contents left.
    pub fn drain(&mut self, used: f64) -> f64 {
        let mut remaining = self.contents - used;
        if remaining >= self.unusable() {
            self.contents -= used;
        } else {
            if self.contents > self.unusable() {
                self.contents = self.unusable();
            }
            remaining = self.contents;
        }
        self.pct_full = 100.0 * self.contents / self.capacity;
        self.calculate_inertias();
        remaining
    }

    /// `Fill`: adds `amount` lbs up to the capacity; what did not fit.
    pub fn fill(&mut self, amount: f64) -> f64 {
        let mut overage = 0.0;
        self.contents += amount;
        if self.contents > self.capacity {
            overage = self.contents - self.capacity;
            self.contents = self.capacity;
            self.pct_full = 100.0;
        } else {
            self.pct_full = self.contents / self.capacity * 100.0;
        }
        self.calculate_inertias();
        overage
    }

    /// `SetContents`, capped at the capacity.
    pub fn set_contents(&mut self, amount: f64) {
        self.contents = amount;
        if self.contents > self.capacity {
            self.contents = self.capacity;
            self.pct_full = 100.0;
        } else {
            self.pct_full = self.contents / self.capacity * 100.0;
        }
        self.calculate_inertias();
    }

    /// `Calculate(dt, TAT_C)`: the external flow over `dt`, then the heat
    /// exchange with air at `tat_c` °C; the temperature, or 0 when the tank
    /// has none.
    pub fn calculate(&mut self, dt: f64, tat_c: f64) -> f64 {
        if self.external_flow < 0.0 {
            self.drain(-self.external_flow * dt);
        } else {
            self.fill(self.external_flow * dt);
        }
        if self.temperature == -9999.0 {
            return 0.0;
        }
        let heat_capacity = 900.0;
        let temp_flow_factor = 1.115;
        let tdiff = tat_c - self.temperature;
        let mut d_temp = 0.0;
        if tdiff.abs() > 0.1 && self.contents > 0.01 {
            d_temp = (temp_flow_factor * self.area * tdiff * dt) / (self.contents * heat_capacity);
        }
        self.temperature += d_temp + d_temp;
        self.temperature
    }

    /// `CalculateInertias`: a solid grain's inertia from its burnt-back
    /// shape, or a liquid's as a shrinking sphere of `radius`.
    ///
    /// JSBSim throws when a grain's density is not positive while it has
    /// contents; the loader refuses such a tank (see [`Tank::load`]), and
    /// should contents be set into one later this logs the error and
    /// leaves the inertias as they were.
    pub fn calculate_inertias(&mut self) {
        if self.grain_density_fault() {
            tracing::error!(
                "Solid propellant grain density is zero! (tank {})",
                self.number
            );
            return;
        }
        let mass = self.contents * LBTOSLUG;
        let rad2 = self.radius * self.radius;
        let g = &mut self.grain;
        if g.kind == GrainType::Unknown {
            if self.radius > 0.0 {
                let i = mass * self.inertia_factor * 0.4 * self.radius * self.radius / 144.0;
                (self.ixx, self.iyy, self.izz) = (i, i, i);
            }
            return;
        }
        g.volume = if self.density > 0.0 {
            (self.contents * LBTOSLUG) / self.density
        } else {
            0.0
        };
        match g.kind {
            GrainType::Cylindrical => {
                g.inner_radius = (rad2 - g.volume / (std::f64::consts::PI * g.length)).sqrt();
                let rad_sum_sqr = (rad2 + g.inner_radius * g.inner_radius) / 144.0;
                self.ixx = 0.5 * mass * rad_sum_sqr;
                self.iyy = mass * (3.0 * rad_sum_sqr + g.length * g.length / 144.0) / 12.0;
                self.izz = self.iyy;
            }
            GrainType::EndBurning => {
                g.length = g.volume / (std::f64::consts::PI * rad2);
                self.ixx = 0.5 * mass * rad2 / 144.0;
                self.iyy = mass * (3.0 * rad2 + g.length * g.length) / (144.0 * 12.0);
                self.izz = self.iyy;
            }
            GrainType::Unknown => {}
        }
    }

    /// Where JSBSim's `CalculateInertias` throws.
    fn grain_density_fault(&self) -> bool {
        let positive = self.density.partial_cmp(&0.0) == Some(std::cmp::Ordering::Greater);
        self.grain.kind != GrainType::Unknown && !positive && self.contents > 0.0
    }
}

/// `ProcessFuelName`: the density of a named fuel, lbs/gal; 6.6, with an
/// error logged, for an unknown name.
#[must_use]
pub fn fuel_density(name: &str) -> f64 {
    match name {
        "AVGAS" => 6.02,
        "JET-A" | "JET-A1" | "F-35" => 6.74,
        "JET-B" | "JP-4" | "F-40" | "AVTAG" => 6.48,
        "JP-1" => 6.76,
        "JP-2" => 6.38,
        "JP-3" => 6.34,
        "JP-5" | "F-44" | "AVCAT" => 6.81,
        "JP-6" => 6.55,
        "JP-7" => 6.61,
        "JP-8" | "JP-8+100" | "F-34" => 6.66,
        "RP-1" => 6.73,
        "T-1" => 6.88,
        "ETHANOL" => 6.58,
        "HYDRAZINE" => 8.61,
        _ => {
            tracing::error!("Unknown fuel type specified: {name}");
            6.6
        }
    }
}

crate::snapshot::visit_fields! { Grain { length, inner_radius, volume } skip { kind } }
crate::snapshot::visit_fields! { Initial { contents, temperature, standpipe, priority } }
crate::snapshot::visit_fields! {
    Tank {
        grain, number, xyz, xyz_drain, capacity, unusable_volume, radius, density, ixx, iyy, izz,
        inertia_factor, pct_full, contents, area, temperature, standpipe, external_flow, selected,
        priority, initial,
    }
    skip { tank_type, name }
}
