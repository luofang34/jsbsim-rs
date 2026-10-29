// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGPropulsion.cpp (Load, Run) and
// src/FGFDMExec.cpp (LoadInputs, LoadModel).
//
//! Loading and running the propulsion system.

use super::order::Model;
use super::{Fdm, Models};
use crate::Error;
use crate::math::ColumnVector3;
use crate::models::external_reactions::Transforms;
use crate::models::propulsion::{self, Tank, TankType, families, tank};
use crate::xml::{DocumentLoader, Element, IncludeKind};

impl Models {
    /// `LoadInputs(ePropulsion)`.
    pub(super) fn load_propulsion_inputs(&mut self) {
        let (at, aux) = (&self.atmosphere, &self.auxiliary);
        let i = &mut self.propulsion.input;
        i.pressure = at.pressure;
        i.pressure_ratio = at.get_pressure_ratio();
        i.temperature = at.temperature;
        i.density_ratio = at.get_density_ratio();
        i.density = at.density;
        i.soundspeed = at.sound_speed;
        i.total_pressure = aux.airspeed.pt;
        i.vc = aux.airspeed.vcas * crate::units::FPSTOKTS;
        i.vt = aux.vt;
        i.qbar = aux.qbar;
        i.tat_c = aux.airspeed.tatc;
        i.aero_uvw = aux.aero_uvw;
        i.aero_pqr = aux.aero_pqr;
        i.alpha = aux.alpha;
        i.beta = aux.beta;
        i.total_delta_t = self.exec.dt * f64::from(self.propulsion.base.rate);
        i.h_agl = self.propagate.distance_agl(&self.inertial);
        i.pqri = self.propagate.state.pqri;
    }
}

impl Fdm {
    /// `FGPropulsion::Load`: the include and model functions, the tanks,
    /// the engines, then the dump and refuel rates and the post-functions.
    pub(super) fn load_propulsion(
        &mut self,
        loader: &mut DocumentLoader<'_>,
        el: &mut Element,
    ) -> Result<(), Error> {
        self.upload(loader, el, IncludeKind::Model, Model::Propulsion)?;
        let mut fuel_density = 6.0;
        for (n, tank_el) in el.find_elements("tank").enumerate() {
            let tank = Tank::load(tank_el, n)?;
            tank::bind(&mut self.env.binder(), n)?;
            let tank_type = tank.tank_type;
            if tank_type == TankType::Fuel {
                fuel_density = tank.density;
            }
            self.models.propulsion.tanks.push(tank);
            if tank_type == TankType::Unknown {
                return Err(Error::Model {
                    message: format!("{}: Unknown tank type specified.", tank_el.location()),
                });
            }
        }
        let mut engines: Vec<Element> = el.find_elements("engine").cloned().collect();
        for (n, engine_el) in engines.iter_mut().enumerate() {
            self.load_engine(loader, engine_el, n)?;
        }
        let p = &mut self.models.propulsion;
        if !p.engines.is_empty() {
            let types: Vec<_> = p.engines.iter().map(|e| e.kind.engine_type()).collect();
            let feeds = p.engines.iter().map(|e| e.base.source_tanks.len()).max();
            p.reserve_feeds(feeds.unwrap_or(0));
            propulsion::bind(&mut self.env.binder(), &types)?;
        }
        let m = &mut self.models;
        m.propulsion.calculate_tank_inertias(&m.mass_balance);
        let p = &mut self.models.propulsion;
        if el.find_element("dump-rate").is_some() {
            p.dump_rate = el.find_element_value_as_number_convert_to("dump-rate", "LBS/MIN")?;
        }
        if el.find_element("refuel-rate").is_some() {
            p.refuel_rate = el.find_element_value_as_number_convert_to("refuel-rate", "LBS/MIN")?;
        }
        for engine in &mut p.engines {
            engine.base.fuel_density = fuel_density;
        }
        self.post_load(el, "", Model::Propulsion)
    }

    /// `FGPropulsion::Run`.
    pub(super) fn run_propulsion(&mut self) -> Result<(), Error> {
        if self.models.propulsion.base.skip() || self.models.exec.holding {
            return Ok(());
        }
        self.run_model_functions(Model::Propulsion, true)?;
        let p = &mut self.models.propulsion;
        p.forces = ColumnVector3::zero();
        p.moments = ColumnVector3::zero();
        for i in 0..self.models.propulsion.engines.len() {
            self.calculate_engine(i)?;
            let trim_status = self.models.exec.trim_status;
            self.models.propulsion.consume_fuel(i, trim_status)?;
            self.sum_engine_forces(i);
        }
        self.models.propulsion.run_tanks();
        self.run_model_functions(Model::Propulsion, false)
    }

    /// `vForces += engine->GetBodyForces(); vMoments += engine->GetMoments()`.
    pub(super) fn sum_engine_forces(&mut self, i: usize) {
        let m = &mut self.models;
        let t = Transforms {
            tw2b: m.auxiliary.tw2b,
            tl2b: m.propagate.frames.tl2b,
            ti2b: m.propagate.frames.ti2b,
        };
        let Some(engine) = m.propulsion.engines.get_mut(i) else {
            return;
        };
        let force = &mut engine.thruster.base.force;
        let body = force.body_forces(&t, &m.mass_balance);
        let moment = force.moment;
        m.propulsion.forces += body;
        m.propulsion.moments += moment;
    }

    /// The engine's `Calculate`: its pre-functions, its type's reads and
    /// update, then its post-functions.
    pub(super) fn calculate_engine(&mut self, i: usize) -> Result<(), Error> {
        self.run_engine_functions(i, true)?;
        families::calculate(&mut self.env, &mut self.models, i)?;
        self.run_engine_functions(i, false)
    }

    fn run_engine_functions(&mut self, i: usize, pre: bool) -> Result<(), Error> {
        let Some(engine) = self.models.propulsion.engines.get(i) else {
            return Ok(());
        };
        let f = &engine.base.functions;
        for &id in if pre { &f.pre } else { &f.post } {
            self.env.cache_value(&self.models, id)?;
        }
        Ok(())
    }
}
