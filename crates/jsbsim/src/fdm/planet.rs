// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.cpp (LoadPlanet) and
// src/input_output/FGPropertyManager.cpp (Unbind).
//
//! Planets: a `<planet>` element or file sets FGInertial's constants,
//! re-initialises the models, and may replace the atmosphere.

use super::order::Model;
use super::{Fdm, ModelFunctions};
use crate::models::atmosphere::{Atmosphere, AtmosphereModel, STANDARD_PROPERTIES};
use crate::xml::{DocumentLoader, Element, IncludeKind};
use crate::{Error, Resolver};

#[cfg(test)]
mod tests;

impl Fdm {
    /// `LoadPlanet(PlanetPath, useAircraftPath)`: the `<planet>` document
    /// at `path` (in the aircraft path when `use_aircraft_path`), loaded
    /// as an aircraft's `<planet>` element is.
    pub fn load_planet_file(
        &mut self,
        resolver: &dyn Resolver,
        path: &str,
        use_aircraft_path: bool,
    ) -> Result<(), Error> {
        let file = match use_aircraft_path {
            true => format!("{}/{path}", self.aircraft_path),
            false => path.to_owned(),
        };
        let mut loader = DocumentLoader::new(resolver, self.full_aircraft_path.clone())
            .with_systems_dir(self.systems_path.clone());
        let mut document = loader.load_document(&file)?;
        if document.name() != "planet" {
            return Err(Error::Model {
                message: format!(
                    "{}: File: {file} is not a planet file.",
                    document.location()
                ),
            });
        }
        self.load_planet(&mut loader, &mut document)
    }

    /// `LoadPlanet(element)`: the planet's constants (a `file` include
    /// merged in first), the models re-initialised with them, then the
    /// atmosphere model the optional `<atmosphere model="...">` names.
    pub(super) fn load_planet(
        &mut self,
        loader: &mut DocumentLoader<'_>,
        el: &mut Element,
    ) -> Result<(), Error> {
        self.upload(loader, el, IncludeKind::Model, Model::Inertial)?;
        self.models.inertial.load(el)?;
        self.load_planet_constants();
        self.models.ic.initialize_ic(&self.models.inertial);
        self.initialize_models()?;
        let Some(atm) = el.find_element("atmosphere") else {
            return Ok(());
        };
        if !atm.has_attribute("model") {
            return Ok(());
        }
        match atm.attribute("model").to_ascii_lowercase().as_str() {
            "mars" => self.replace_atmosphere(Atmosphere::mars(), None),
            "msis" => {
                let atm = atm.clone();
                self.replace_atmosphere(Atmosphere::msis(), Some((loader, atm)))
            }
            _ => Ok(()),
        }
    }

    /// The new atmosphere model of `LoadPlanet`: the old one is unbound
    /// (its properties untied, keeping their values) and the new one
    /// binds `FGAtmosphere`'s properties again, and the standard
    /// atmosphere's only if it is one; it is initialised, loads its
    /// element (`FGMSIS` its date, `FGMars` nothing), and the models are
    /// initialised again.
    fn replace_atmosphere(
        &mut self,
        mut atmosphere: Atmosphere,
        msis: Option<(&mut DocumentLoader<'_>, Element)>,
    ) -> Result<(), Error> {
        // `FGMSIS` is a standard atmosphere and binds them again.
        if atmosphere.model == AtmosphereModel::Mars {
            for path in STANDARD_PROPERTIES {
                self.untie_property(path)?;
            }
        }
        // The enable flag is tied to a pointer, which `Unbind` leaves in
        // place, so the flag stays with the replaced model and the new
        // model's own tie of it fails: the new model is always enabled.
        let old = &self.models.atmosphere;
        atmosphere.orphaned_enabled = Some(old.orphaned_enabled.unwrap_or(old.base.enabled));
        tracing::error!(
            "Failed to tie property simulation/models/FGAtmosphere/enabled to a pointer"
        );
        atmosphere.sim_time = self.models.exec.sim_time;
        self.models.atmosphere = atmosphere;
        self.model_functions[Model::Atmosphere] = ModelFunctions::default();
        self.models.load_inputs(Model::Atmosphere);
        self.init_atmosphere()?;
        if let Some((loader, mut el)) = msis {
            self.upload(loader, &mut el, IncludeKind::Model, Model::Atmosphere)?;
            self.models.atmosphere.load_msis(&el)?;
        }
        self.initialize_models()
    }

    /// `PropertyState::untie`: the node at `path` keeps the value its
    /// getter gives now and becomes a plain property.
    fn untie_property(&mut self, path: &str) -> Result<(), Error> {
        let Some(node) = self.env.tree.get(path)? else {
            return Ok(());
        };
        if !self.env.bindings.is_bound(node) {
            return Ok(());
        }
        let value = self.read_node(node)?;
        self.env.bindings.unbind(node);
        self.env.tree.untie(node);
        self.env.tree.set_double(node, value);
        Ok(())
    }
}
