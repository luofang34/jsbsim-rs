// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGPropulsion.cpp (Load's engine loop)
// and src/models/propulsion/FGEngine.cpp (Load, LoadThruster).
//
//! Loading the engines and their thrusters.

use super::Fdm;
use super::order::Model;
use crate::Error;
use crate::functions::FnId;
use crate::models::propulsion::engine::{Engine, EngineBase, EngineType};
use crate::models::propulsion::families::{
    self, ENGINE_ELEMENTS, EngineHost, THRUSTER_ELEMENTS, ThrusterSite,
};
use crate::models::propulsion::force::TransformType;
use crate::models::propulsion::thruster::{Thruster, ThrusterBase};
use crate::models::propulsion::{self as propulsion, Inputs};
use crate::models::{LoadContext, ModelFunctions};
use crate::xml::{DocumentLoader, Element, IncludeKind};

/// The [`EngineHost`] of one `<engine>`: the executive, the element and the
/// engine's number.
struct EngineLoad<'a> {
    fdm: &'a mut Fdm,
    parent: &'a Element,
    number: usize,
    loaded: Option<(EngineBase, Thruster)>,
}

impl EngineHost for EngineLoad<'_> {
    fn number(&self) -> usize {
        self.number
    }

    fn context(&mut self) -> LoadContext<'_> {
        let options = self.fdm.options;
        self.fdm.env.load_context(options)
    }

    fn function_value(&mut self, id: FnId) -> Result<f64, Error> {
        let fdm = &mut *self.fdm;
        fdm.env.eval(&fdm.models, |ev| ev.function(id))
    }

    fn inputs(&self) -> &Inputs {
        &self.fdm.models.propulsion.input
    }

    fn loaded_mut(&mut self) -> Option<(&mut EngineBase, &mut Thruster, &Inputs)> {
        let input = &self.fdm.models.propulsion.input;
        self.loaded
            .as_mut()
            .map(|(base, thruster)| (base, thruster, input))
    }

    fn base_mut(&mut self) -> Option<&mut EngineBase> {
        self.loaded.as_mut().map(|(base, _)| base)
    }

    fn load_base(&mut self, el: &Element, engine_type: EngineType) -> Result<(), Error> {
        let n = self.number;
        let prefix = n.to_string();
        let mut base = EngineBase::new(n, engine_type);
        base.name = el.attribute("name").to_owned();
        base.functions = self.fdm.engine_functions(el, &prefix, false)?;
        if self.parent.find_element("location").is_some() {
            tracing::warn!("Engine location ignored, only thruster location is used.");
        }
        if self.parent.find_element("orient").is_some() {
            tracing::warn!("Engine orientation ignored, only thruster orientation is used.");
        }
        let Some(thruster_el) = self.parent.find_element("thruster") else {
            return Err(self
                .parent
                .model_error("No thruster definition supplied with engine definition."));
        };
        let mut thruster = self.fdm.load_thruster(thruster_el, n)?;
        base.reset_to_ic();
        thruster.reset_to_ic();
        for feed in self.parent.find_elements("feed") {
            base.source_tanks.push(feed.data_as_number()? as i32);
        }
        propulsion::bind_engine(&mut self.fdm.env.binder(), n)?;
        let post = self.fdm.engine_functions(el, &prefix, true)?;
        base.functions.post = post.post;
        self.loaded = Some((base, thruster));
        Ok(())
    }

    fn take_loaded(
        &mut self,
        engine_type: EngineType,
    ) -> Result<(EngineBase, Thruster, &mut Vec<Engine>), Error> {
        let Some((mut base, thruster)) = self.loaded.take() else {
            return Err(self
                .parent
                .model_error("the engine type did not load its FGEngine base"));
        };
        base.engine_type = engine_type;
        Ok((base, thruster, &mut self.fdm.models.propulsion.engines))
    }
}

impl Fdm {
    /// `FGModelFunctions::Load` (or, with `post`, `PostLoad`) of an engine
    /// element with `prefix`: the propulsion's own functions are set aside
    /// while the loader fills the slot.
    fn engine_functions(
        &mut self,
        el: &Element,
        prefix: &str,
        post: bool,
    ) -> Result<ModelFunctions, Error> {
        let own = std::mem::take(&mut self.model_functions[Model::Propulsion]);
        let loaded = if post {
            self.post_load(el, prefix, Model::Propulsion)
        } else {
            self.load_model_functions(el, prefix, Model::Propulsion)
        };
        let engine = std::mem::replace(&mut self.model_functions[Model::Propulsion], own);
        loaded.map(|()| engine)
    }

    /// `FGEngine::LoadThruster`: the first thruster element JSBSim knows,
    /// loaded by its registered type.
    fn load_thruster(&mut self, thruster_el: &Element, n: usize) -> Result<Thruster, Error> {
        #[cfg(test)]
        let names = THRUSTER_ELEMENTS
            .iter()
            .chain(&families::TEST_THRUSTER_ELEMENTS);
        #[cfg(not(test))]
        let names = THRUSTER_ELEMENTS.iter();
        for &name in names {
            let Some(el) = thruster_el.find_element(name) else {
                continue;
            };
            if !families::thruster_registered(name) {
                return Err(el.unsupported(format!("<{name}>")));
            }
            let mut base = ThrusterBase {
                name: el.attribute("name").to_owned(),
                engine_number: n,
                gear_ratio: 1.0,
                ..ThrusterBase::default()
            };
            base.force.transform_type = TransformType::Custom;
            base.load(thruster_el, el, &mut self.env.binder())?;
            let options = self.options;
            let ctx = &mut self.env.load_context(options);
            let site = ThrusterSite {
                dt: self.models.exec.dt,
                engines: &self.models.propulsion.engines,
            };
            return families::load_thruster(name, el, thruster_el, base, ctx, &site)?
                .ok_or_else(|| el.unsupported(format!("<{name}>")));
        }
        Err(thruster_el.model_error("Unknown thruster type"))
    }

    /// `FGPropulsion::Load`'s engine loop for `<engine>` number `n`: its
    /// file and its thruster's are opened, then its type constructs it.
    pub(super) fn load_engine(
        &mut self,
        loader: &mut DocumentLoader<'_>,
        engine_el: &mut Element,
        n: usize,
    ) -> Result<(), Error> {
        loader.open(engine_el, IncludeKind::Engine)?;
        let Some(thruster) = engine_el
            .children_mut()
            .iter_mut()
            .find(|c| c.name() == "thruster")
        else {
            return Err(
                engine_el.model_error("No thruster definition supplied with engine definition.")
            );
        };
        loader.open(thruster, IncludeKind::Engine)?;
        #[cfg(test)]
        let names = ENGINE_ELEMENTS
            .iter()
            .chain(&families::TEST_ENGINE_ELEMENTS);
        #[cfg(not(test))]
        let names = ENGINE_ELEMENTS.iter();
        for &name in names {
            let Some(el) = engine_el.find_element(name) else {
                continue;
            };
            let mut ctx = EngineLoad {
                fdm: self,
                parent: engine_el,
                number: n,
                loaded: None,
            };
            if families::load_engine(name, el, &mut ctx)? {
                return Ok(());
            }
            return Err(el.unsupported(format!("<{name}>")));
        }
        Err(engine_el.model_error("Unknown engine type"))
    }
}
