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
use crate::models::propulsion::{Tank, TankType, tank};
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
        for (n, tank_el) in el.find_elements("tank").enumerate() {
            let tank = Tank::load(tank_el, n)?;
            tank::bind(&mut self.env.binder(), n)?;
            let unknown = tank.tank_type == TankType::Unknown;
            self.models.propulsion.tanks.push(tank);
            if unknown {
                return Err(Error::Model {
                    message: format!("{}: Unknown tank type specified.", tank_el.location()),
                });
            }
        }
        if let Some(engine) = el.find_element("engine") {
            return Err(Error::Unsupported {
                construct: "<engine> in <propulsion>".to_owned(),
                path: engine.location().to_string(),
            });
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
        p.run_tanks();
        self.run_model_functions(Model::Propulsion, false)
    }
}
