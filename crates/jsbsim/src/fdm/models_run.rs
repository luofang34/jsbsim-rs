// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGFCS.cpp, FGAerodynamics.cpp,
// FGExternalReactions.cpp and FGGroundReactions.cpp (Load and Run), and
// src/math/FGModelFunctions.cpp (RunPreFunctions, RunPostFunctions,
// InitModel).
//
//! Models whose `Run` evaluates functions: they read the property tree
//! through getters, so their functions are evaluated first and the
//! results applied to the model afterwards.

use super::order::Model;
use super::{Fdm, ModelGetters};
use crate::Error;
use crate::functions::Evaluator;
use crate::math::ColumnVector3;
use crate::models::aerodynamics::Values;
use crate::models::external_reactions::{Magnitude, Transforms};
use crate::models::fcs::FcsFunction;
use crate::models::ground_reactions;
use crate::properties::Attribute;
use crate::units::constrain;
use crate::xml::{DocumentLoader, Element, IncludeKind};

impl Fdm {
    /// `RunPreFunctions` / `RunPostFunctions` of `model`: caches each
    /// function's value for the frame.
    pub(super) fn run_model_functions(&mut self, model: Model, pre: bool) -> Result<(), Error> {
        let mf = &self.model_functions[model as usize];
        let ids = if pre { mf.pre.clone() } else { mf.post.clone() };
        for id in ids {
            let getters = ModelGetters {
                bindings: &self.bindings,
                models: &self.models,
            };
            self.functions
                .cache_value_with(id, true, &mut self.tree, &getters)?;
        }
        Ok(())
    }

    /// `FGModelFunctions::InitModel`: declared properties back to their
    /// initial values unless persistent.
    pub(super) fn reset_local_properties(&mut self, model: Model) -> Result<(), Error> {
        let props: Vec<_> = self.model_functions[model as usize]
            .local_properties
            .iter()
            .map(|(n, v)| (*n, *v))
            .collect();
        for (node, value) in props {
            if !self.tree.attribute(node, Attribute::PRESERVE) {
                self.write_node(node, value)?;
            }
        }
        Ok(())
    }

    pub(super) fn load_ground_reactions(&mut self, el: &Element) -> Result<(), Error> {
        self.models.ground_reactions.load(el)?;
        self.post_load(el, "", Model::GroundReactions)
    }

    pub(super) fn load_external_reactions(&mut self, el: &Element) -> Result<(), Error> {
        let mut external = std::mem::take(&mut self.models.external_reactions);
        let result = external.load(el, &mut self.context());
        self.models.external_reactions = external;
        result
    }

    pub(super) fn load_aerodynamics(&mut self, el: &Element) -> Result<(), Error> {
        let mut aero = std::mem::take(&mut self.models.aerodynamics);
        let result = aero.load(el, &mut self.context());
        self.models.aerodynamics = aero;
        result
    }

    /// `FGFCS::Load` for one `<system>`, `<autopilot>` or `<flight_control>`.
    pub(super) fn load_fcs(
        &mut self,
        loader: &mut DocumentLoader<'_>,
        el: &mut Element,
        kind: IncludeKind,
    ) -> Result<(), Error> {
        self.upload(loader, el, kind, Model::Systems)?;
        let mut fcs = std::mem::take(&mut self.models.fcs);
        let result = fcs.load_channels(el, &mut self.context());
        self.models.fcs = fcs;
        result?;
        self.post_load(el, "", Model::Systems)
    }

    /// `LoadModelConstants`.
    pub(super) fn load_model_constants(&mut self) {
        let m = &mut self.models;
        m.winds.input.wingspan = m.aircraft.wing_span;
        let a = &mut m.aerodynamics.input;
        a.wing_area = m.aircraft.wing_area;
        a.wing_chord = m.aircraft.cbar;
        a.wing_incidence = m.aircraft.wing_incidence;
        a.wing_span = m.aircraft.wing_span;
        let c = &mut m.auxiliary.input.constants;
        c.wingspan = m.aircraft.wing_span;
        c.wingchord = m.aircraft.cbar;
    }

    /// `FGFCS::Run`.
    pub(super) fn run_fcs(&mut self) -> Result<(), Error> {
        if self.models.fcs.base.skip() || self.models.exec.holding {
            return Ok(());
        }
        self.run_model_functions(Model::Systems, true)?;
        for c in 0..self.models.fcs.channels.len() {
            self.run_channel(c)?;
        }
        self.models.fcs.channel_rate = 1;
        self.run_model_functions(Model::Systems, false)
    }

    fn run_channel(&mut self, c: usize) -> Result<(), Error> {
        let channel = &self.models.fcs.channels[c];
        self.models.fcs.channel_rate = channel.exec_rate;
        if let Some(node) = channel.on_off {
            let getters = ModelGetters {
                bindings: &self.bindings,
                models: &self.models,
            };
            let on = Evaluator::with_getters(&self.functions, &mut self.tree, &getters)
                .read_node(node)?
                != 0.0;
            if !on {
                return Ok(());
            }
        }
        let m = &self.models;
        let (dt, trim) = (m.fcs.dt(m.exec.dt), m.exec.trim_status);
        if !self.models.fcs.channels[c].due(dt, trim) {
            return Ok(());
        }
        for i in 0..self.models.fcs.channels[c].components.len() {
            let component = self.models.fcs.channels[c].components[i].clone();
            let output = self.evaluate_fcs_function(&component)?;
            self.models.fcs.channels[c].components[i].output = output;
            for node in component.outputs {
                self.write_node(node, output)?;
            }
        }
        Ok(())
    }

    /// `FGFCSFunction::Run` up to `SetOutput`.
    fn evaluate_fcs_function(&mut self, component: &FcsFunction) -> Result<f64, Error> {
        let getters = ModelGetters {
            bindings: &self.bindings,
            models: &self.models,
        };
        let mut ev = Evaluator::with_getters(&self.functions, &mut self.tree, &getters);
        let mut output = ev.function(component.function)?;
        if let Some(input) = component.inputs.first() {
            output *= ev.property(input)?;
        }
        if let Some(clip) = &component.clip {
            let vmin = ev.parameter(&clip.min)?;
            let vmax = ev.parameter(&clip.max)?;
            let range = vmax - vmin;
            if range < 0.0 {
                tracing::error!(
                    "Trying to clip {} with a max value lower than the min value. Clipping is ignored.",
                    component.name
                );
            } else if clip.cyclic && range != 0.0 {
                output = libm::fmod(output - vmin, range) + vmin;
                if output < vmin {
                    output += range;
                }
            } else {
                output = constrain(vmin, output, vmax);
            }
        }
        Ok(output)
    }

    /// `FGAerodynamics::Run`.
    pub(super) fn run_aerodynamics(&mut self) -> Result<(), Error> {
        if self.models.aerodynamics.base.skip() || self.models.exec.holding {
            return Ok(());
        }
        self.models.aerodynamics.begin_frame();
        self.run_model_functions(Model::Aerodynamics, true)?;
        self.models.aerodynamics.prepare();
        let aero = &self.models.aerodynamics;
        let (functions, at_cg, rp_shift) = (
            aero.functions.clone(),
            aero.functions_at_cg.clone(),
            aero.rp_shift,
        );
        let mut values = Values::default();
        for axis in 0..3 {
            for id in &functions[axis] {
                values.native[axis + 1] += self.cached_value(*id)?;
            }
            for id in &at_cg[axis] {
                values.native_at_cg[axis + 1] += self.cached_value(*id)?;
            }
        }
        if let Some(id) = rp_shift {
            let getters = ModelGetters {
                bindings: &self.bindings,
                models: &self.models,
            };
            values.rp_shift = Some(
                Evaluator::with_getters(&self.functions, &mut self.tree, &getters).function(id)?,
            );
        }
        for axis in 0..3 {
            for id in &functions[axis + 3] {
                values.moments_mrc[axis + 1] += self.cached_value(*id)?;
            }
        }
        self.models.aerodynamics.finish(&values)?;
        self.run_model_functions(Model::Aerodynamics, false)
    }

    /// `cacheValue(true)` followed by `GetValue()`.
    fn cached_value(&mut self, id: crate::functions::FnId) -> Result<f64, Error> {
        let getters = ModelGetters {
            bindings: &self.bindings,
            models: &self.models,
        };
        self.functions
            .cache_value_with(id, true, &mut self.tree, &getters)?;
        Ok(self.functions.function(id).cached().unwrap_or(f64::NAN))
    }

    /// `FGExternalReactions::Run`.
    pub(super) fn run_external_reactions(&mut self) -> Result<(), Error> {
        let m = &mut self.models;
        if m.external_reactions.base.skip()
            || m.exec.holding
            || m.external_reactions.forces.is_empty()
        {
            return Ok(());
        }
        self.run_model_functions(Model::ExternalReactions, true)?;
        let mut read = Vec::with_capacity(self.models.external_reactions.forces.len());
        for f in &self.models.external_reactions.forces {
            let getters = ModelGetters {
                bindings: &self.bindings,
                models: &self.models,
            };
            let mut ev = Evaluator::with_getters(&self.functions, &mut self.tree, &getters);
            let magnitude = match f.magnitude {
                Magnitude::Function(id) => ev.function(id)?,
                Magnitude::Property(node) => ev.read_node(node)?,
            };
            let mut direction = ColumnVector3::zero();
            for (i, node) in f.direction.iter().enumerate() {
                direction[i + 1] = ev.read_node(*node)?;
            }
            read.push((magnitude, direction));
        }
        let m = &mut self.models;
        let t = Transforms {
            tw2b: m.auxiliary.tw2b,
            tl2b: m.propagate.frames.tl2b,
            ti2b: m.propagate.frames.ti2b,
        };
        let er = &mut m.external_reactions;
        er.total_forces = ColumnVector3::zero();
        er.total_moments = ColumnVector3::zero();
        for (f, (magnitude, direction)) in er.forces.iter_mut().zip(read) {
            f.update(magnitude, direction, &t, |r| {
                m.mass_balance.structural_to_body(r)
            });
            er.total_forces += f.body_force;
            er.total_moments += f.moment;
        }
        self.run_model_functions(Model::ExternalReactions, false)
    }

    /// `FGGroundReactions::Run`: no gear forces yet; a touching contact is
    /// an error.
    pub(super) fn run_ground_reactions(&mut self) -> Result<(), Error> {
        let m = &mut self.models;
        if m.ground_reactions.base.skip() || m.exec.holding {
            return Ok(());
        }
        let input = ground_reactions::Inputs {
            location: m.propagate.state.location.clone(),
            tb2l: m.propagate.frames.tb2l,
            xyz_cg: m.mass_balance.xyz_cg,
            fcs_gear_pos: m.fcs.gear_pos,
        };
        let (trim, time) = (m.exec.trim_status, m.exec.sim_time);
        if m.ground_reactions.surface.bumpiness >= 0.001 && !trim {
            return Err(Error::Unsupported {
                construct: "ground/bumpiness (FGSurface bumps)".to_owned(),
                path: "ground_reactions".to_owned(),
            });
        }
        self.run_model_functions(Model::GroundReactions, true)?;
        let m = &mut self.models;
        m.ground_reactions
            .check_contacts(&input, &m.inertial, trim, time)?;
        self.run_model_functions(Model::GroundReactions, false)
    }
}
