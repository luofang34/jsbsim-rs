// SPDX-License-Identifier: LGPL-2.1-or-later
// FGGasCell: Copyright (C) 2006 - 2013 Anders Gidenstam.
// Derived from JSBSim src/models/FGGasCell.{h,cpp} (FGGasCell).
//
//! A gas cell: a volume of lifting gas whose temperature, pressure and
//! contents follow the ideal gas law, giving buoyancy, mass and inertia.

use super::ballonet::Ballonet;
use super::shape::{Envelope, c_max, c_min};
use super::{GAS_R, Inputs, M_AIR};
use crate::Error;
use crate::fdm::LoadContext;
use crate::functions::FnId;
use crate::math::{ColumnVector3, E_X, E_Y, E_Z, Matrix33};
use crate::models::mass_balance::MassBalance;
use crate::units::SLUGTOLB;
use crate::xml::Element;

/// `FGGasCell::M_hydrogen`, slug/mol.
const M_HYDROGEN: f64 = 0.00013841;
/// `FGGasCell::M_helium`, slug/mol.
const M_HELIUM: f64 = 0.00027409;

/// `GasType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GasType {
    /// `ttUNKNOWN`: behaves as air.
    Unknown,
    /// `ttHYDROGEN`.
    Hydrogen,
    /// `ttHELIUM`.
    Helium,
    /// `ttAIR`.
    Air,
}

impl GasType {
    fn parse(name: &str) -> Self {
        match name {
            "HYDROGEN" => Self::Hydrogen,
            "HELIUM" => Self::Helium,
            "AIR" => Self::Air,
            _ => Self::Unknown,
        }
    }

    /// `M_gas`, slug/mol.
    #[must_use]
    pub fn molar_mass(self) -> f64 {
        match self {
            Self::Hydrogen => M_HYDROGEN,
            Self::Helium => M_HELIUM,
            Self::Air | Self::Unknown => M_AIR,
        }
    }

    /// `Cv_gas`.
    #[must_use]
    pub fn cv(self) -> f64 {
        match self {
            Self::Helium => 3.0 / 2.0,
            Self::Hydrogen | Self::Air | Self::Unknown => 5.0 / 2.0,
        }
    }
}

/// `FGGasCell`.
#[derive(Debug, Clone)]
pub struct GasCell {
    /// The gas.
    pub gas: GasType,
    /// Location, shape and envelope.
    pub envelope: Envelope,
    /// `Temperature`, °R.
    pub temperature: f64,
    /// `Pressure`, psf.
    pub pressure: f64,
    /// `Contents`, mol.
    pub contents: f64,
    /// `Volume`, ft^3, ballonets included.
    pub volume: f64,
    /// `dVolumeIdeal`, ft^3.
    pub d_volume_ideal: f64,
    /// `Buoyancy`, lbs.
    pub buoyancy: f64,
    /// `ValveOpen`, 0..1.
    pub valve_open: f64,
    /// `Mass`, slugs, ballonets included.
    pub mass: f64,
    /// Heat transfer functions (`HeatTransferCoeff`), lbs·ft/s.
    pub heat: Vec<FnId>,
    /// The ballonets.
    pub ballonets: Vec<Ballonet>,
    /// `gasCellJ`: inertia about the CG, body frame, slug·ft^2.
    pub inertia: Matrix33,
    /// `gasCellM`: moment of the mass, structural frame, lbs·in.
    pub mass_moment: ColumnVector3,
    /// `vFb` of the last frame, lbs.
    pub body_force: ColumnVector3,
    /// `vM` of the last frame, lbs·ft.
    pub moment: ColumnVector3,
}

/// What `Calculate` keeps from before the ballonets update.
#[derive(Debug, Clone, Copy)]
pub struct Started {
    old_temperature: f64,
    old_pressure: f64,
}

impl GasCell {
    /// `FGGasCell(exec, el, num, in)` up to its ties: the state starts
    /// from the inputs as they stand when the cell is loaded.
    pub fn load(el: &Element, input: &Inputs) -> Result<Self, Error> {
        let gas = GasType::parse(el.attribute("type"));
        let envelope = Envelope::load(el, "gas cell")?;
        let temperature = input.temperature;
        let (contents, pressure) = envelope.initial_contents(GAS_R, temperature, input.pressure);
        let volume = contents * GAS_R * temperature / pressure;
        Ok(Self {
            gas,
            envelope,
            temperature,
            pressure,
            contents,
            volume,
            d_volume_ideal: 0.0,
            buoyancy: 0.0,
            valve_open: 0.0,
            mass: contents * gas.molar_mass(),
            heat: Vec::new(),
            ballonets: Vec::new(),
            inertia: Matrix33::zero(),
            mass_moment: ColumnVector3::zero(),
            body_force: ColumnVector3::zero(),
            moment: ColumnVector3::zero(),
        })
    }

    /// The `<heat>` functions, read after the ties.
    pub fn load_heat(&mut self, el: &Element, ctx: &mut LoadContext<'_>) -> Result<(), Error> {
        if let Some(heat) = el.find_element("heat") {
            for f in heat.find_elements("function") {
                self.heat
                    .push(ctx.functions.load_function(ctx.binder.tree, f, "")?);
            }
        }
        Ok(())
    }

    /// `Calculate` up to the ballonets' update: temperature from the heat
    /// flow `du` (`None` without heat functions), pressure, and manual
    /// valving.
    pub fn start(&mut self, du: Option<f64>, input: &Inputs, dt: f64) -> Started {
        let started = Started {
            old_temperature: self.temperature,
            old_pressure: self.pressure,
        };
        let env = self.envelope;
        let mut ballonets_volume = 0.0;
        let mut ballonets_heat_flow = 0.0;
        for b in &self.ballonets {
            ballonets_volume += b.volume;
            ballonets_heat_flow += b.du;
        }

        match du {
            Some(du) if self.contents > 0.0 => {
                self.temperature +=
                    (du * dt - self.pressure * self.d_volume_ideal - ballonets_heat_flow)
                        / (self.gas.cv() * self.contents * GAS_R);
            }
            _ => self.temperature = input.temperature,
        }

        let ideal_pressure =
            self.contents * GAS_R * self.temperature / (env.max_volume - ballonets_volume);
        if ideal_pressure > input.pressure + env.max_overpressure {
            self.pressure = input.pressure + env.max_overpressure;
        } else {
            self.pressure = c_max(ideal_pressure, input.pressure);
        }

        if env.valve_coefficient > 0.0 && self.valve_open > 0.0 {
            // The cell's height shrinks with the gas it holds, so the
            // buoyancy-driven pressure difference fades as it deflates.
            let gas_volume = self.contents * GAS_R * self.temperature / self.pressure;
            let shape = env.shape;
            let cell_height =
                (2.0 * shape.z_radius + shape.z_width) * c_min(1.0, gas_volume / env.max_volume);
            let m_gas = self.gas.molar_mass();
            let gas_density = m_gas * self.pressure / (GAS_R * self.temperature);
            let delta_pressure = c_max(
                0.0,
                self.pressure + cell_height * input.gravity * (input.density - gas_density)
                    - input.pressure,
            );
            let volume_valved = self.valve_open * env.valve_coefficient * delta_pressure * dt;
            self.contents = c_max(
                0.0,
                self.contents - self.pressure * volume_valved / (GAS_R * self.temperature),
            );
        }
        started
    }

    /// `Calculate` after the ballonets' update: safety valving, volume,
    /// buoyancy, mass, inertia and the moment of the mass; then
    /// `GetBodyForces` with the local-to-body transform `tl2b`.
    pub fn finish(
        &mut self,
        started: Started,
        input: &Inputs,
        mass_balance: &MassBalance,
        tl2b: &Matrix33,
    ) {
        let env = self.envelope;
        let mut ballonets_volume = 0.0;
        for b in &self.ballonets {
            ballonets_volume += b.volume;
        }
        if self.contents * GAS_R * self.temperature / (env.max_volume - ballonets_volume)
            > input.pressure + env.max_overpressure
        {
            // Valving capacity is taken to be infinite.
            self.contents = (input.pressure + env.max_overpressure)
                * (env.max_volume - ballonets_volume)
                / (GAS_R * self.temperature);
        }

        self.volume = self.contents * GAS_R * self.temperature / self.pressure + ballonets_volume;
        self.d_volume_ideal = self.contents
            * GAS_R
            * (self.temperature / self.pressure - started.old_temperature / started.old_pressure);

        // Gross buoyancy: the gas's own weight reaches FGMassBalance as mass.
        self.buoyancy = self.volume * input.density * input.gravity;
        let force_local = ColumnVector3::new(0.0, 0.0, -self.buoyancy);

        let mass = self.contents * self.gas.molar_mass();
        let mut j = Matrix33::zero();
        let (ixx, iyy, izz) = env.shape.inertia(mass);
        j[(E_X, E_X)] = ixx;
        j[(E_Y, E_Y)] = iyy;
        j[(E_Z, E_Z)] = izz;
        self.mass = mass;
        j += mass_balance.pointmass_inertia(self.mass, env.xyz);

        let mut m = ColumnVector3::zero();
        for axis in [E_X, E_Y, E_Z] {
            m[axis] += env.xyz[axis] * self.mass * SLUGTOLB;
        }
        for b in &self.ballonets {
            self.mass += b.mass();
            for axis in [E_X, E_Y, E_Z] {
                m[axis] += b.envelope.xyz[axis] * b.mass() * SLUGTOLB;
            }
            j += b.inertia;
        }
        self.inertia = j;
        self.mass_moment = m;

        self.body_force = *tl2b * force_local;
        let arm = mass_balance.structural_to_body(env.xyz);
        self.moment = ColumnVector3::zero() + arm.cross(self.body_force);
    }
}
