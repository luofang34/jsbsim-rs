// SPDX-License-Identifier: LGPL-2.1-or-later
// FGBallonet: Copyright (C) 2006 - 2013 Anders Gidenstam.
// Derived from JSBSim src/models/FGGasCell.{h,cpp} (FGBallonet).
//
//! A ballonet: an air bag inside a gas cell, inflated by a blower and
//! vented through a valve, that keeps the cell's envelope taut.

use super::shape::{Envelope, c_max};
use super::{GAS_R, Inputs, M_AIR};
use crate::Error;
use crate::fdm::LoadContext;
use crate::functions::FnId;
use crate::math::{E_X, E_Y, E_Z, Matrix33};
use crate::models::mass_balance::MassBalance;
use crate::xml::Element;

/// `FGBallonet::Cv_air`.
const CV_AIR: f64 = 5.0 / 2.0;

/// `FGBallonet`.
#[derive(Debug, Clone)]
pub struct Ballonet {
    /// Location, shape and envelope.
    pub envelope: Envelope,
    /// `Temperature`, °R.
    pub temperature: f64,
    /// `Pressure`, psf.
    pub pressure: f64,
    /// `Contents`, mol.
    pub contents: f64,
    /// `Volume`, ft^3.
    pub volume: f64,
    /// `dVolumeIdeal`, ft^3.
    pub d_volume_ideal: f64,
    /// `dU`: the heat flow of the last frame, lbs·ft/s.
    pub du: f64,
    /// `ValveOpen`, 0..1.
    pub valve_open: f64,
    /// Heat transfer functions (`HeatTransferCoeff`), lbs·ft/s.
    pub heat: Vec<FnId>,
    /// `BlowerInput`, ft^3/s.
    pub blower: Option<FnId>,
    /// `ballonetJ`: inertia about the CG, body frame, slug·ft^2.
    pub inertia: Matrix33,
}

/// What `Calculate` keeps from before the blower input is read.
#[derive(Debug, Clone, Copy)]
pub struct Started {
    old_temperature: f64,
    old_pressure: f64,
}

impl Ballonet {
    /// `FGBallonet(exec, el, num, parent, in)` without the ties: the
    /// parent's temperature and pressure start it.
    pub fn load(
        el: &Element,
        parent_temperature: f64,
        parent_pressure: f64,
    ) -> Result<Self, Error> {
        let envelope = Envelope::load(el, "ballonet")?;
        let temperature = parent_temperature;
        let (contents, pressure) = envelope.initial_contents(GAS_R, temperature, parent_pressure);
        let volume = contents * GAS_R * temperature / pressure;
        Ok(Self {
            envelope,
            temperature,
            pressure,
            contents,
            volume,
            d_volume_ideal: 0.0,
            du: 0.0,
            valve_open: 0.0,
            heat: Vec::new(),
            blower: None,
            inertia: Matrix33::zero(),
        })
    }

    /// The `<heat>` functions, then the `<blower_input>` function, as the
    /// constructor reads them after tying the properties.
    pub fn load_functions(&mut self, el: &Element, ctx: &mut LoadContext<'_>) -> Result<(), Error> {
        if let Some(heat) = el.find_element("heat") {
            for f in heat.find_elements("function") {
                self.heat
                    .push(ctx.functions.load_function(ctx.binder.tree, f, "")?);
            }
        }
        if let Some(blower) = el.find_element("blower_input") {
            let Some(f) = blower.find_element("function") else {
                return Err(Error::Model {
                    message: format!("{}: <blower_input> has no function", blower.location()),
                });
            };
            self.blower = Some(ctx.functions.load_function(ctx.binder.tree, f, "")?);
        }
        Ok(())
    }

    /// `GetMass`, slugs.
    #[must_use]
    pub fn mass(&self) -> f64 {
        self.contents * M_AIR
    }

    /// `FGBallonet::Calculate(dt)` up to the blower input: temperature
    /// from the heat flow `du`, then pressure, given the parent's
    /// temperature and pressure after the parent's own update.
    pub fn start(&mut self, du: f64, parent: (f64, f64), dt: f64) -> Started {
        let (parent_temperature, parent_pressure) = parent;
        let started = Started {
            old_temperature: self.temperature,
            old_pressure: self.pressure,
        };
        self.du = du;
        if self.contents > 0.0 {
            self.temperature += (self.du * dt - self.pressure * self.d_volume_ideal)
                / (CV_AIR * self.contents * GAS_R);
        } else {
            self.temperature = parent_temperature;
        }
        let ideal_pressure = self.contents * GAS_R * self.temperature / self.envelope.max_volume;
        self.pressure = c_max(ideal_pressure, parent_pressure);
        started
    }

    /// The rest of `Calculate`: the blower's `blower` ft^3/s, valving,
    /// volume and inertia.
    pub fn finish(
        &mut self,
        started: Started,
        blower: Option<f64>,
        input: &Inputs,
        mass_balance: &MassBalance,
        dt: f64,
    ) {
        let air_pressure = input.pressure;
        let env = self.envelope;
        if let Some(blower) = blower {
            let added_volume = blower * dt;
            if added_volume > 0.0 {
                self.contents += self.pressure * added_volume / (GAS_R * self.temperature);
            }
        }

        let relief = self.pressure > air_pressure + env.max_overpressure;
        if env.valve_coefficient > 0.0 && (self.valve_open > 0.0 || relief) {
            let delta_pressure = self.pressure - air_pressure;
            let opening = if relief { 1.0 } else { self.valve_open };
            let volume_valved = opening * env.valve_coefficient * delta_pressure * dt;
            // JSBSim keeps at least one mole, since fewer sometimes gives NaN.
            self.contents = c_max(
                1.0,
                self.contents - self.pressure * volume_valved / (GAS_R * self.temperature),
            );
        }

        self.volume = self.contents * GAS_R * self.temperature / self.pressure;
        self.d_volume_ideal = self.contents
            * GAS_R
            * (self.temperature / self.pressure - started.old_temperature / started.old_pressure);

        let mut j = Matrix33::zero();
        let (ixx, iyy, izz) = env.shape.inertia(self.contents * M_AIR);
        j[(E_X, E_X)] = ixx;
        j[(E_Y, E_Y)] = iyy;
        j[(E_Z, E_Z)] = izz;
        j += mass_balance.pointmass_inertia(self.mass(), env.xyz);
        self.inertia = j;
    }
}

crate::snapshot::visit_fields! {
    Ballonet {
        envelope, temperature, pressure, contents, volume, d_volume_ideal, du, valve_open, inertia,
    }
    skip { heat, blower }
}
