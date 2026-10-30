// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.cpp (the GetIC()->Load calls of
// LoadScript and the Python interface's load_ic).
//
//! Loading initial conditions into the executive.

use super::Fdm;
use crate::Error;

impl Fdm {
    /// `FGInitialCondition::Load(file)` for `dir/name` (`.xml` added when
    /// it has no extension), applied to the executive's initial conditions. False, with the
    /// problem logged, where JSBSim's `Load` returns false.
    pub fn load_initial_conditions(
        &mut self,
        resolver: &dyn crate::Resolver,
        dir: &str,
        name: &str,
    ) -> Result<bool, Error> {
        self.hashed_load(resolver, |fdm, r| {
            fdm.load_initial_conditions_from(r, dir, name)
        })
    }

    /// [`load_initial_conditions`](Self::load_initial_conditions) within
    /// another load.
    pub(super) fn load_initial_conditions_from(
        &mut self,
        resolver: &dyn crate::Resolver,
        dir: &str,
        name: &str,
    ) -> Result<bool, Error> {
        let loader = crate::xml::DocumentLoader::new(resolver, dir);
        let document = loader.load_document(&format!("{dir}/{name}"))?;
        self.load_initial_conditions_document(&document)
    }

    /// `FGInitialCondition::Load` on a parsed `<initialize>` document.
    pub(crate) fn load_initial_conditions_document(
        &mut self,
        document: &crate::xml::Element,
    ) -> Result<bool, Error> {
        let m = &mut self.models;
        let outcome = m.ic.load(document, &m.atmosphere, &mut m.inertial);
        self.loaded_without_documents();
        let outcome = outcome?;
        if outcome.planet_rotation_changed {
            let m = &mut self.models;
            m.propagate.input.omega_planet = m.inertial.omega_planet;
            m.accelerations.input.omega_planet = m.inertial.omega_planet;
        }
        Ok(outcome.result)
    }
}
