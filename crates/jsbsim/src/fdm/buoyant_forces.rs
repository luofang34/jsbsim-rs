// SPDX-License-Identifier: LGPL-2.1-or-later
// FGBuoyantForces: Copyright (C) 2006 - 2013 Anders Gidenstam.
// Derived from JSBSim src/models/FGBuoyantForces.cpp (Load, Run) and
// src/models/FGGasCell.cpp (Calculate).
//
//! Loading and running the gas cells, whose heat and blower functions
//! read the cells' own properties between the steps of their update.

use super::Fdm;
use super::order::Model;
use crate::Error;
use crate::math::ColumnVector3;
use crate::models::buoyant_forces::Inputs;
use crate::xml::{DocumentLoader, Element, IncludeKind};

impl Fdm {
    /// `FGBuoyantForces::Load`.
    pub(super) fn load_buoyant_forces(
        &mut self,
        loader: &mut DocumentLoader<'_>,
        el: &mut Element,
    ) -> Result<(), Error> {
        self.upload(loader, el, IncludeKind::Model, Model::BuoyantForces)?;
        let ctx = &mut self.env.load_context(self.options);
        self.models.buoyant_forces.load(el, ctx)?;
        self.post_load(el, "", Model::BuoyantForces)?;
        let b = &mut self.env.binder();
        self.models.buoyant_forces.bind(b)
    }

    /// The sum of the heat functions of cell `i` or, with `k`, of its
    /// ballonet `k`, evaluated in load order.
    fn heat_flow(&mut self, i: usize, k: Option<usize>) -> Result<f64, Error> {
        let models = &self.models;
        self.env.eval(models, |ev| {
            let cell = models.buoyant_forces.cells.get(i);
            let heat = match k {
                None => cell.map(|c| &c.heat),
                Some(k) => cell.and_then(|c| c.ballonets.get(k)).map(|b| &b.heat),
            };
            let mut du = 0.0;
            for &id in heat.into_iter().flatten() {
                du += ev.function(id)?;
            }
            Ok(du)
        })
    }

    /// `FGBuoyantForces::Run`.
    pub(super) fn run_buoyant_forces(&mut self) -> Result<(), Error> {
        let m = &mut self.models;
        if m.buoyant_forces.base.skip() || m.exec.holding || m.buoyant_forces.none_defined {
            return Ok(());
        }
        self.run_model_functions(Model::BuoyantForces, true)?;
        let bf = &mut self.models.buoyant_forces;
        bf.forces = ColumnVector3::zero();
        bf.moments = ColumnVector3::zero();
        for i in 0..bf.cells.len() {
            self.run_gas_cell(i)?;
            let bf = &mut self.models.buoyant_forces;
            if let Some(c) = bf.cells.get(i) {
                bf.forces += c.body_force;
                bf.moments += c.moment;
            }
        }
        self.run_model_functions(Model::BuoyantForces, false)
    }

    /// `FGGasCell::Calculate(dt)` and `GetBodyForces` of cell `i`.
    fn run_gas_cell(&mut self, i: usize) -> Result<(), Error> {
        let dt = self.models.exec.dt;
        let Some(cell) = self.models.buoyant_forces.cells.get(i) else {
            return Ok(());
        };
        let du = match cell.heat.is_empty() {
            true => None,
            false => Some(self.heat_flow(i, None)?),
        };
        let bf = &mut self.models.buoyant_forces;
        let input = bf.input;
        let Some(cell) = bf.cells.get_mut(i) else {
            return Ok(());
        };
        let started = cell.start(du, &input, dt);
        for k in 0..cell.ballonets.len() {
            self.run_ballonet(i, k, &input, dt)?;
        }
        let m = &mut self.models;
        if let Some(cell) = m.buoyant_forces.cells.get_mut(i) {
            cell.finish(started, &input, &m.mass_balance, &m.propagate.frames.tl2b);
        }
        Ok(())
    }

    /// `FGBallonet::Calculate(dt)` of ballonet `k` of cell `i`; its blower
    /// function is read once its temperature and pressure are updated.
    fn run_ballonet(&mut self, i: usize, k: usize, input: &Inputs, dt: f64) -> Result<(), Error> {
        let du = self.heat_flow(i, Some(k))?;
        let Some(cell) = self.models.buoyant_forces.cells.get_mut(i) else {
            return Ok(());
        };
        let parent = (cell.temperature, cell.pressure);
        let Some(b) = cell.ballonets.get_mut(k) else {
            return Ok(());
        };
        let started = b.start(du, parent, dt);
        let blower = match b.blower {
            Some(id) => Some(self.env.eval(&self.models, |ev| ev.function(id))?),
            None => None,
        };
        let m = &mut self.models;
        if let Some(b) = m
            .buoyant_forces
            .cells
            .get_mut(i)
            .and_then(|c| c.ballonets.get_mut(k))
        {
            b.finish(started, blower, input, &m.mass_balance, dt);
        }
        Ok(())
    }
}
