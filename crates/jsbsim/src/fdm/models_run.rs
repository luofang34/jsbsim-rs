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

use super::Fdm;
use super::order::Model;
use crate::Error;
use crate::functions::Evaluator;
use crate::math::ColumnVector3;
use crate::models::aerodynamics::Values;
use crate::models::external_reactions::{Magnitude, Transforms};
use crate::models::fcs::{Fcs, FcsFunction};
use crate::properties::{Attribute, NodeId};
use crate::units::constrain;
use crate::xml::{DocumentLoader, Element, IncludeKind};

impl Fdm {
    /// `RunPreFunctions` / `RunPostFunctions` of `model`: caches each
    /// function's value for the frame.
    pub(super) fn run_model_functions(&mut self, model: Model, pre: bool) -> Result<(), Error> {
        let mf = &self.model_functions[model as usize];
        let ids = if pre { &mf.pre } else { &mf.post };
        for &id in ids {
            self.env.cache_value(&self.models, id)?;
        }
        Ok(())
    }

    /// `FGModelFunctions::InitModel`: declared properties back to their
    /// initial values unless persistent.
    pub(super) fn reset_local_properties(&mut self, model: Model) -> Result<(), Error> {
        // A write may reach a setter, so each entry is copied out by slot
        // rather than borrowed across it.
        let slots = self.model_functions[model as usize]
            .local_properties
            .slot_count();
        for slot in 0..slots {
            let props = &self.model_functions[model as usize].local_properties;
            let Some((node, &value)) = props.entry(slot) else {
                continue;
            };
            if !self.env.tree.attribute(node, Attribute::PRESERVE) {
                self.write_node(node, value)?;
            }
        }
        Ok(())
    }

    pub(super) fn load_ground_reactions(&mut self, el: &Element) -> Result<(), Error> {
        let ctx = &mut self.env.load_context(self.options);
        self.models.ground_reactions.load(el, ctx)?;
        self.post_load(el, "", Model::GroundReactions)
    }

    pub(super) fn load_external_reactions(&mut self, el: &Element) -> Result<(), Error> {
        let ctx = &mut self.env.load_context(self.options);
        self.models.external_reactions.load(el, ctx)
    }

    pub(super) fn load_aerodynamics(&mut self, el: &Element) -> Result<(), Error> {
        let ctx = &mut self.env.load_context(self.options);
        self.models.aerodynamics.load(el, ctx)
    }

    /// `FGFCS::Load` for one `<system>`, `<autopilot>` or `<flight_control>`.
    pub(super) fn load_fcs(
        &mut self,
        loader: &mut DocumentLoader<'_>,
        el: &mut Element,
        kind: IncludeKind,
    ) -> Result<(), Error> {
        self.upload(loader, el, kind, Model::Systems)?;
        let ctx = &mut self.env.load_context(self.options);
        self.models.fcs.load_channels(el, ctx)?;
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
        m.ground_reactions.input.xyz_cg = m.mass_balance.xyz_cg;
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
            let on = self.env.eval(&self.models, |ev| ev.read_node(node))? != 0.0;
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
            let component = &self.models.fcs.channels[c].components[i];
            let output = self
                .env
                .eval(&self.models, |ev| evaluate_fcs_function(ev, component))?;
            self.models.fcs.channels[c].components[i].output = output;
            // A setter may reach any model, and its request may reset the
            // executive, so each output node is looked up afresh instead of
            // being borrowed across the write.
            let mut k = 0;
            while let Some(node) = output_node(&self.models.fcs, c, i, k) {
                self.write_node(node, output)?;
                k += 1;
            }
        }
        Ok(())
    }

    /// `FGAerodynamics::Run`.
    pub(super) fn run_aerodynamics(&mut self) -> Result<(), Error> {
        if self.models.aerodynamics.base.skip() || self.models.exec.holding {
            return Ok(());
        }
        self.models.aerodynamics.begin_frame();
        self.run_model_functions(Model::Aerodynamics, true)?;
        self.models.aerodynamics.prepare();
        let (env, models) = (&mut self.env, &self.models);
        let aero = &models.aerodynamics;
        let mut values = Values::default();
        for axis in 0..3 {
            for &id in &aero.functions[axis] {
                values.native[axis + 1] += env.cached_value(models, id)?;
            }
            for &id in &aero.functions_at_cg[axis] {
                values.native_at_cg[axis + 1] += env.cached_value(models, id)?;
            }
        }
        if let Some(id) = aero.rp_shift {
            values.rp_shift = Some(env.eval(models, |ev| ev.function(id))?);
        }
        for axis in 0..3 {
            for &id in &aero.functions[axis + 3] {
                values.moments_mrc[axis + 1] += env.cached_value(models, id)?;
            }
        }
        self.models.aerodynamics.finish(&values)?;
        self.run_model_functions(Model::Aerodynamics, false)
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
        let m = &mut self.models;
        let t = Transforms {
            tw2b: m.auxiliary.tw2b,
            tl2b: m.propagate.frames.tl2b,
            ti2b: m.propagate.frames.ti2b,
        };
        let er = &mut m.external_reactions;
        er.total_forces = ColumnVector3::zero();
        er.total_moments = ColumnVector3::zero();
        // Each force reads its inputs and is summed before the next one
        // reads, as `GetBodyForces` does inside JSBSim's loop.
        for i in 0..self.models.external_reactions.forces.len() {
            let f = &self.models.external_reactions.forces[i];
            let (magnitude, direction) = self.env.eval(&self.models, |ev| {
                let magnitude = match f.magnitude {
                    Magnitude::Function(id) => ev.function(id)?,
                    Magnitude::Property(node) => ev.read_node(node)?,
                };
                let mut direction = ColumnVector3::zero();
                for (axis, node) in f.direction.iter().enumerate() {
                    direction[axis + 1] = ev.read_node(*node)?;
                }
                Ok((magnitude, direction))
            })?;
            let m = &mut self.models;
            let er = &mut m.external_reactions;
            let f = &mut er.forces[i];
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
        let (trim, time) = (m.exec.trim_status, m.exec.sim_time);
        if m.ground_reactions.surface.bumpiness >= 0.001 && !trim {
            return Err(Error::Unsupported {
                construct: "ground/bumpiness (FGSurface bumps)".to_owned(),
                path: "ground_reactions".to_owned(),
            });
        }
        self.run_model_functions(Model::GroundReactions, true)?;
        let m = &mut self.models;
        m.ground_reactions.forces = ColumnVector3::zero();
        m.ground_reactions.moments = ColumnVector3::zero();
        m.ground_reactions.check_contacts(&m.inertial, trim, time)?;
        self.run_model_functions(Model::GroundReactions, false)
    }
}

/// Output node `k` of component `i` of channel `c`.
fn output_node(fcs: &Fcs, c: usize, i: usize, k: usize) -> Option<NodeId> {
    let component = fcs.channels.get(c)?.components.get(i)?;
    component.outputs.get(k).copied()
}

/// `FGFCSFunction::Run` up to `SetOutput`.
fn evaluate_fcs_function(ev: &mut Evaluator<'_>, component: &FcsFunction) -> Result<f64, Error> {
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
