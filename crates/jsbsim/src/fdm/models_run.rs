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

use crate::math::{Axis, ColumnVector3};
use crate::models::aerodynamics::Values;
use crate::models::external_reactions::{Magnitude, Transforms};
use crate::models::fcs::{ComponentAt, Fcs, families};
use crate::models::ground_reactions::FrameContext;
use crate::properties::{Attribute, NodeId};

use crate::xml::{DocumentLoader, Element, IncludeKind};

impl Fdm {
    /// `RunPreFunctions` / `RunPostFunctions` of `model`: caches each
    /// function's value for the frame.
    pub(super) fn run_model_functions(&mut self, model: Model, pre: bool) -> Result<(), Error> {
        let mf = &self.model_functions[model];
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
        let slots = self.model_functions[model].local_properties.slot_count();
        for slot in 0..slots {
            let props = &self.model_functions[model].local_properties;
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
        let exec_dt = self.models.exec.dt;
        ctx.ic_position = Some(*self.models.ic.position());
        self.models.fcs.load_channels(el, ctx, exec_dt)?;
        let m = &mut self.models;
        crate::models::fcs::magnetometer::construct(&mut m.fcs, &m.propagate, &m.exec);
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
        self.models.fcs.throttles.follow_commands();
        for c in 0..self.models.fcs.channels.len() {
            self.run_channel(c)?;
        }
        self.models.fcs.channel_rate = 1;
        self.run_model_functions(Model::Systems, false)
    }

    fn run_channel(&mut self, c: usize) -> Result<(), Error> {
        let Some(channel) = self.models.fcs.channels.get(c) else {
            return Ok(());
        };
        self.models.fcs.channel_rate = channel.exec_rate;
        if let Some(node) = channel.on_off {
            let on = self.env.eval(&self.models, |ev| ev.read_node(node))? != 0.0;
            if !on {
                return Ok(());
            }
        }
        let m = &self.models;
        let (dt, trim) = (m.fcs.dt(m.exec.dt), m.exec.trim_status);
        if !self
            .models
            .fcs
            .channels
            .get_mut(c)
            .is_some_and(|ch| ch.due(dt, trim))
        {
            return Ok(());
        }
        let mut at = ComponentAt {
            channel: c,
            index: 0,
        };
        while families::run(&mut self.env, &mut self.models, at)? {
            self.run_distributor(at)?;
            // `SetOutput`. A setter may reach any model, and its request may
            // reset the executive, so each output node is looked up afresh
            // instead of being borrowed across the write.
            let mut k = 0;
            while let Some((node, output)) = output_node(&self.models.fcs, at, k) {
                self.write_node(node, output)?;
                k += 1;
            }
            at.index += 1;
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
        let [fx, fy, fz, mx, my, mz] = &aero.functions;
        let (forces, moments) = ([fx, fy, fz], [mx, my, mz]);
        for ((ids, at_cg), axis) in forces.into_iter().zip(&aero.functions_at_cg).zip(Axis::ALL) {
            for &id in ids {
                values.native[axis] += env.cached_value(models, id)?;
            }
            for &id in at_cg {
                values.native_at_cg[axis] += env.cached_value(models, id)?;
            }
        }
        if let Some(id) = aero.rp_shift {
            values.rp_shift = Some(env.eval(models, |ev| ev.function(id))?);
        }
        for (ids, axis) in moments.into_iter().zip(Axis::ALL) {
            for &id in ids {
                values.moments_mrc[axis] += env.cached_value(models, id)?;
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
        let mut i = 0;
        while let Some(f) = self.models.external_reactions.forces.get(i) {
            let (magnitude, direction) = self.env.eval(&self.models, |ev| {
                let magnitude = match f.magnitude {
                    Magnitude::Function(id) => ev.function(id)?,
                    Magnitude::Property(node) => ev.read_node(node)?,
                };
                let mut direction = ColumnVector3::zero();
                for (&node, axis) in f.direction.iter().zip(Axis::ALL) {
                    direction[axis] = ev.read_node(node)?;
                }
                Ok((magnitude, direction))
            })?;
            let m = &mut self.models;
            let er = &mut m.external_reactions;
            let Some(f) = er.forces.get_mut(i) else {
                break;
            };
            f.update(magnitude, direction, &t, |r| {
                m.mass_balance.structural_to_body(r)
            });
            er.total_forces += f.body_force;
            er.total_moments += f.moment;
            i += 1;
        }
        self.run_model_functions(Model::ExternalReactions, false)
    }

    /// `FGGroundReactions::Run`: every contact's forces, in load order.
    pub(super) fn run_ground_reactions(&mut self) -> Result<(), Error> {
        let m = &mut self.models;
        if m.ground_reactions.base.skip() || m.exec.holding {
            return Ok(());
        }
        self.run_model_functions(Model::GroundReactions, true)?;
        let gr = &mut self.models.ground_reactions;
        gr.forces = ColumnVector3::zero();
        gr.moments = ColumnVector3::zero();
        gr.multipliers.clear();
        for index in 0..gr.contacts.len() {
            self.run_contact(index)?;
        }
        let m = &self.models;
        if let Some(c) = m.ground_reactions.contacts.iter().find(|c| c.state.wow) {
            return Err(Error::Touchdown {
                contact: c.name.clone(),
                time: m.exec.sim_time,
            });
        }
        self.run_model_functions(Model::GroundReactions, false)
    }

    /// `FGLGear::GetBodyForces` of contact `index`, summed into the
    /// model's force and moment. A `<strut_force>` function is evaluated
    /// once the contact's compression and its speed are set, since it may
    /// read them.
    fn run_contact(&mut self, index: usize) -> Result<(), Error> {
        let m = &mut self.models;
        let frame = FrameContext {
            trim_status: m.exec.trim_status,
            sim_time: m.exec.sim_time,
            integration_suspended: m.exec.integration_suspended(),
        };
        let gr = &mut m.ground_reactions;
        let (surface, bump) = (gr.surface, gr.surface.bump_height());
        let Some(contact) = gr.contacts.get_mut(index) else {
            return Ok(());
        };
        let compressed =
            contact.begin_frame(&gr.input, &m.inertial, &surface, bump, frame.trim_status);
        let function = contact.strut.function;
        if let Some(compressed) = compressed {
            let strut = match function {
                Some(id) => Some(self.env.eval(&self.models, |ev| ev.function(id))?),
                None => None,
            };
            let gr = &mut self.models.ground_reactions;
            if let Some(contact) = gr.contacts.get_mut(index) {
                let registry = &mut gr.multipliers;
                let functions = &self.env.functions;
                contact
                    .finish_compressed(compressed, strut, &gr.input, functions, registry, index)?;
            }
        }
        let m = &mut self.models;
        let gr = &mut m.ground_reactions;
        if let Some(contact) = gr.contacts.get_mut(index) {
            let mass_balance = &m.mass_balance;
            contact.end_frame(&gr.input, &frame, |r| mass_balance.structural_to_body(r));
            gr.forces += contact.force.body_force;
            gr.moments += contact.force.moment;
        }
        Ok(())
    }
}

/// Output node `k` of the component at `at`, with the component's output.
fn output_node(fcs: &Fcs, at: ComponentAt, k: usize) -> Option<(NodeId, f64)> {
    let component = fcs.channels.get(at.channel)?.components.get(at.index)?;
    let node = component.base.outputs.get(k).copied()?;
    Some((node, component.base.output))
}
