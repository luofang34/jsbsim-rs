// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGPropulsion.cpp (Load's engine loop)
// and src/models/propulsion/FGEngine.cpp (Load, LoadThruster).
//
//! Loading the engines and their thrusters.

use super::order::Model;
use super::{Fdm, LoadContext, ModelFunctions};
use crate::Error;
use crate::models::propulsion::engine::{Engine, EngineBase, EngineType};
use crate::models::propulsion::families::{self, ENGINE_ELEMENTS, THRUSTER_ELEMENTS};
use crate::models::propulsion::force::TransformType;
use crate::models::propulsion::thruster::{Thruster, ThrusterBase};
use crate::models::propulsion::{self as propulsion};
use crate::xml::{DocumentLoader, Element, IncludeKind};

/// What an engine type's constructor ([`families::EngineFamily::load`])
/// works with: the executive, the `<engine>` element and the engine's
/// number.
pub struct EngineLoad<'a> {
    fdm: &'a mut Fdm,
    parent: &'a Element,
    number: usize,
    loaded: Option<(EngineBase, Thruster)>,
}

impl EngineLoad<'_> {
    /// The engine's index among the propulsion's engines.
    #[must_use]
    pub fn number(&self) -> usize {
        self.number
    }

    /// The `<engine>` element (with any engine file merged into it).
    #[must_use]
    pub fn parent(&self) -> &Element {
        self.parent
    }

    /// Functions, tables and ties, for what the type itself loads.
    pub fn context(&mut self) -> LoadContext<'_> {
        let options = self.fdm.options;
        self.fdm.env.load_context(options)
    }

    /// The engine's base, once [`EngineLoad::load_base`] has loaded it.
    pub fn base_mut(&mut self) -> Option<&mut EngineBase> {
        self.loaded.as_mut().map(|(base, _)| base)
    }

    /// The engine's thruster, once [`EngineLoad::load_base`] has loaded it.
    pub fn thruster_mut(&mut self) -> Option<&mut Thruster> {
        self.loaded.as_mut().map(|(_, thruster)| thruster)
    }

    /// `FGEngine::Load(exec, el)` for the type's element `el`: the engine's
    /// model functions (prefixed with its number), its thruster, the reset
    /// of the base and the thruster, the feed tanks and the
    /// `propulsion/engine[n]/` properties, then the post-functions. A type
    /// whose `ResetToIC` resets state of its own resets it where it calls
    /// this.
    pub fn load_base(&mut self, el: &Element, engine_type: EngineType) -> Result<(), Error> {
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
            return Err(Error::Model {
                message: format!(
                    "{}: No thruster definition supplied with engine definition.",
                    self.parent.location()
                ),
            });
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

    /// For the `engines!` registry: has `make` append the engine it builds
    /// from what [`EngineLoad::load_base`] loaded to the engines.
    pub fn push(
        &mut self,
        engine_type: EngineType,
        make: impl FnOnce(EngineBase, Thruster, &mut Vec<Engine>),
    ) -> Result<(), Error> {
        let Some((mut base, thruster)) = self.loaded.take() else {
            return Err(Error::Model {
                message: format!(
                    "{}: the engine type did not load its FGEngine base",
                    self.parent.location()
                ),
            });
        };
        base.engine_type = engine_type;
        make(base, thruster, &mut self.fdm.models.propulsion.engines);
        Ok(())
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
            let mut base = ThrusterBase {
                name: el.attribute("name").to_owned(),
                engine_number: n,
                gear_ratio: 1.0,
                ..ThrusterBase::default()
            };
            base.force.transform_type = TransformType::Custom;
            let options = self.options;
            let ctx = &mut self.env.load_context(options);
            return match families::load_thruster(name, el, base, ctx)? {
                Some(thruster) => Ok(thruster),
                None => Err(Error::Unsupported {
                    construct: format!("<{name}>"),
                    path: el.location().to_string(),
                }),
            };
        }
        Err(Error::Model {
            message: format!("{}: Unknown thruster type", thruster_el.location()),
        })
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
            return Err(Error::Model {
                message: format!(
                    "{}: No thruster definition supplied with engine definition.",
                    engine_el.location()
                ),
            });
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
            let mut cx = EngineLoad {
                fdm: self,
                parent: engine_el,
                number: n,
                loaded: None,
            };
            if families::load_engine(name, el, &mut cx)? {
                return Ok(());
            }
            return Err(Error::Unsupported {
                construct: format!("<{name}>"),
                path: el.location().to_string(),
            });
        }
        Err(Error::Model {
            message: format!("{}: Unknown engine type", engine_el.location()),
        })
    }
}
