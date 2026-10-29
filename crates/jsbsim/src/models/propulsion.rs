// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGPropulsion.{h,cpp} and the inputs of
// src/models/propulsion/FGEngine.h.
//
//! The propulsion system (`FGPropulsion`): its engines and their
//! thrusters, its tanks, the fuel fed and moved between them, and what
//! they contribute to the forces and the mass balance. [`families`] says
//! how engine and thruster types are registered; an engine or thruster
//! no type registers yet is refused with
//! [`Error::Unsupported`](crate::Error::Unsupported).

mod bind;
pub mod engine;
pub mod families;
pub mod force;
mod fuel;
pub mod piston;
pub mod propeller;
pub mod tank;
#[cfg(test)]
mod test_engine;
#[cfg(test)]
mod tests;
pub mod thruster;

pub use bind::{bind, bind_engine};
pub use engine::{Engine, EngineBase, EngineType};
pub use tank::{Tank, TankType};
pub use thruster::{Thruster, ThrusterBase};

use super::base::ModelBase;
use super::mass_balance::MassBalance;
use crate::math::{ColumnVector3, E_X, E_Y, E_Z, Matrix33};
use crate::units::LBTOSLUG;

/// `FGEngine::Inputs`, which the executive loads before each run.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// Static pressure, psf.
    pub pressure: f64,
    /// Pressure over sea-level pressure.
    pub pressure_ratio: f64,
    /// Temperature, °R.
    pub temperature: f64,
    /// Density, slug/ft^3.
    pub density: f64,
    /// Density over sea-level density.
    pub density_ratio: f64,
    /// Speed of sound, ft/s.
    pub soundspeed: f64,
    /// Total pressure, psf.
    pub total_pressure: f64,
    /// Total air temperature, °C.
    pub tat_c: f64,
    /// True airspeed, ft/s.
    pub vt: f64,
    /// Calibrated airspeed, kt.
    pub vc: f64,
    /// Dynamic pressure, psf.
    pub qbar: f64,
    /// Angle of attack, rad.
    pub alpha: f64,
    /// Sideslip, rad.
    pub beta: f64,
    /// Height above ground, ft.
    pub h_agl: f64,
    /// Aero-relative body velocity, ft/s.
    pub aero_uvw: ColumnVector3,
    /// Aero-relative body rates, rad/s.
    pub aero_pqr: ColumnVector3,
    /// Inertial body rates, rad/s.
    pub pqri: ColumnVector3,
    /// Throttle commands, by engine (`FGFCS::GetThrottleCmd`).
    pub throttle_cmd: Vec<f64>,
    /// Mixture commands, by engine.
    pub mixture_cmd: Vec<f64>,
    /// Throttle positions, by engine.
    pub throttle_pos: Vec<f64>,
    /// Mixture positions, by engine.
    pub mixture_pos: Vec<f64>,
    /// Propeller advance positions, by engine.
    pub prop_advance: Vec<f64>,
    /// Propeller feather positions, by engine.
    pub prop_feather: Vec<bool>,
    /// The executive step times the model's rate, s.
    pub total_delta_t: f64,
}

/// `FGPropulsion`.
#[derive(Debug, Clone)]
pub struct Propulsion {
    /// Scheduling.
    pub base: ModelBase,
    /// Inputs.
    pub input: Inputs,
    /// Engines in load order.
    pub engines: Vec<Engine>,
    /// Tanks in load order.
    pub tanks: Vec<Tank>,
    /// The engine starter, cutoff and magneto commands reach: -1 for all.
    pub active_engine: i32,
    /// Force, body frame, lbs.
    pub forces: ColumnVector3,
    /// Moment about the CG, body frame, lbs·ft.
    pub moments: ColumnVector3,
    /// `propulsion/refuel`.
    pub refuel: bool,
    /// `propulsion/fuel_dump`.
    pub dump: bool,
    /// `propulsion/fuel_freeze`.
    pub fuel_freeze: bool,
    /// Fuel in every fuel tank after the last run, lbs.
    pub total_fuel_quantity: f64,
    /// Oxidizer in every oxidizer tank after the last run, lbs.
    pub total_oxidizer_quantity: f64,
    /// Dump rate, lbs/min.
    pub dump_rate: f64,
    /// Refuel rate, lbs/min.
    pub refuel_rate: f64,
    /// Tank inertia of the last [`Propulsion::calculate_tank_inertias`].
    pub tank_j: Matrix33,
    /// `ConsumeFuel`'s feed lists, kept so feeding allocates nothing.
    feed: fuel::FeedLists,
}

impl Default for Propulsion {
    /// `FGPropulsion()`.
    fn default() -> Self {
        Self {
            base: ModelBase::default(),
            input: Inputs::default(),
            engines: Vec::new(),
            tanks: Vec::new(),
            active_engine: -1,
            forces: ColumnVector3::zero(),
            moments: ColumnVector3::zero(),
            refuel: false,
            dump: false,
            fuel_freeze: false,
            total_fuel_quantity: 0.0,
            total_oxidizer_quantity: 0.0,
            dump_rate: 0.0,
            refuel_rate: 6000.0,
            tank_j: Matrix33::zero(),
            feed: fuel::FeedLists::default(),
        }
    }
}

impl Propulsion {
    /// `InitModel`.
    pub fn init_model(&mut self) {
        self.base.init();
        self.forces = ColumnVector3::zero();
        self.moments = ColumnVector3::zero();
        for tank in &mut self.tanks {
            tank.reset_to_ic();
        }
        self.total_fuel_quantity = 0.0;
        self.total_oxidizer_quantity = 0.0;
        self.refuel = false;
        self.dump = false;
        for engine in &mut self.engines {
            engine
                .kind
                .reset_to_ic(&mut engine.base, &mut engine.thruster, &self.input);
        }
    }

    /// Room for engines fed from up to `n` tanks each.
    pub(crate) fn reserve_feeds(&mut self, n: usize) {
        self.feed.reserve(n);
    }

    /// The tanks' part of `Run`, after the engines: each tank's flow and
    /// temperature, the totals, then refuelling and dumping.
    pub fn run_tanks(&mut self) {
        let (dt, tat_c) = (self.input.total_delta_t, self.input.tat_c);
        self.total_fuel_quantity = 0.0;
        self.total_oxidizer_quantity = 0.0;
        for tank in &mut self.tanks {
            tank.calculate(dt, tat_c);
            match tank.tank_type {
                TankType::Fuel => self.total_fuel_quantity += tank.contents,
                TankType::Oxidizer => self.total_oxidizer_quantity += tank.contents,
                TankType::Unknown => {}
            }
        }
        if self.refuel {
            self.do_refuel(dt);
        }
        if self.dump {
            self.dump_fuel(dt);
        }
    }

    /// `Transfer`: drains `amount` lbs from tank `source` into tank
    /// `target` (`None` for outside the aircraft); what did not fit.
    pub fn transfer(&mut self, source: Option<usize>, target: Option<usize>, amount: f64) -> f64 {
        let shortage = source
            .and_then(|s| self.tanks.get_mut(s))
            .map_or(0.0, |t| t.drain(amount));
        target
            .and_then(|t| self.tanks.get_mut(t))
            .map_or(0.0, |t| t.fill(amount - shortage))
    }

    /// `DoRefuel`: fills every tank not yet full equally, over
    /// `time_slice` seconds.
    pub fn do_refuel(&mut self, time_slice: f64) {
        let fillrate = self.refuel_rate / 60.0 * time_slice;
        let not_full = self.tanks.iter().filter(|t| t.pct_full < 99.99).count();
        if not_full == 0 {
            return;
        }
        let per_tank = fillrate / not_full as f64;
        for i in 0..self.tanks.len() {
            if self.tanks.get(i).is_some_and(|t| t.pct_full < 99.99) {
                self.transfer(None, Some(i), per_tank);
            }
        }
    }

    /// `DumpFuel`: drains every tank above its standpipe equally, over
    /// `time_slice` seconds.
    pub fn dump_fuel(&mut self, time_slice: f64) {
        let above = |t: &Tank| t.contents > t.standpipe;
        let dumping = self.tanks.iter().filter(|t| above(t)).count();
        if dumping == 0 {
            return;
        }
        let per_tank = self.dump_rate / 60.0 * time_slice / dumping as f64;
        for i in 0..self.tanks.len() {
            if self.tanks.get(i).is_some_and(above) {
                self.transfer(Some(i), None, per_tank);
            }
        }
    }

    /// `GetTanksWeight`, lbs.
    #[must_use]
    pub fn tanks_weight(&self) -> f64 {
        self.tanks
            .iter()
            .map(|t| t.contents)
            .fold(0.0, |w, c| w + c)
    }

    /// `GetTanksMoment`: the tanks' contents times their CG, lbs·in.
    #[must_use]
    pub fn tanks_moment(&self) -> ColumnVector3 {
        let mut arm = ColumnVector3::zero();
        for tank in &self.tanks {
            arm += tank.xyz() * tank.contents;
        }
        arm
    }

    /// `GetPropulsionTankReport`: a line per tank of its contents (lb),
    /// CG (in) and local inertias (slug·ft^2).
    pub fn tank_report(&mut self, mass_balance: &MassBalance) -> String {
        self.calculate_tank_inertias(mass_balance);
        let mut out = String::new();
        for (i, t) in self.tanks.iter().enumerate() {
            let kind = match (t.tank_type, t.grain.kind) {
                (TankType::Fuel, tank::GrainType::Unknown) => "Fuel",
                (TankType::Fuel, _) => "Solid Fuel",
                (TankType::Oxidizer, _) => "Oxidizer",
                (TankType::Unknown, _) => "Unknown tank type",
            };
            let desc = if t.name.is_empty() {
                kind.to_owned()
            } else {
                format!("{} ({kind})", t.name)
            };
            let xyz = t.xyz();
            out.push_str(&format!(
                "{i:<4}{desc:<30}{:>12.1}{:>8.1}{:>8.1}{:>8.1}{:>12.1}{:>12.1}{:>12.1}\n",
                t.contents, xyz[E_X], xyz[E_Y], xyz[E_Z], t.ixx, t.iyy, t.izz
            ));
        }
        out
    }

    /// `CalculateTankInertias`: every tank's contents as a point mass at
    /// its CG, plus its local inertia, about the CG of `mass_balance`.
    pub fn calculate_tank_inertias(&mut self, mass_balance: &MassBalance) -> Matrix33 {
        if self.tanks.is_empty() {
            return self.tank_j;
        }
        let mut j = Matrix33::zero();
        for tank in &self.tanks {
            j += mass_balance.pointmass_inertia(LBTOSLUG * tank.contents, tank.xyz());
            j[(E_X, E_X)] += tank.ixx;
            j[(E_Y, E_Y)] += tank.iyy;
            j[(E_Z, E_Z)] += tank.izz;
        }
        self.tank_j = j;
        j
    }
}
