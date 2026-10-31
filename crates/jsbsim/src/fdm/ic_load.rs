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
    /// it has no extension), applied to the executive's initial
    /// conditions. False, with the problem logged, where JSBSim's `Load`
    /// returns false. For a file named by one path, such as one at the
    /// resolver's root, use
    /// [`load_initial_conditions_file`](Self::load_initial_conditions_file).
    ///
    /// # Errors
    ///
    /// [`Error::ResourceNotFound`] when the resolver lacks a document;
    /// [`Error::Xml`] for malformed XML or a value that does not parse;
    /// [`Error::Model`] for a configuration JSBSim refuses;
    /// [`Error::Function`] for an invalid table, function or condition;
    /// [`Error::Unsupported`] for a construct the port does not implement;
    /// [`Error::UnknownInputType`] for an `<input>` without an input class;
    /// [`Error::Geographic`] for a planet GeographicLib refuses; and
    /// [`Error::PropertyPath`] for a property name SimGear would reject.
    /// A failed load is not rolled back.
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

    /// `FGInitialCondition::Load(path, false)`: the initialization file at
    /// `path`, relative to the resolver's root, with `.xml` added when it
    /// has no extension. Unlike
    /// [`load_initial_conditions`](Self::load_initial_conditions), a file at
    /// the root needs no directory, which there would read `/name`.
    ///
    /// # Errors
    ///
    /// Those of [`load_initial_conditions`](Self::load_initial_conditions).
    pub fn load_initial_conditions_file(
        &mut self,
        resolver: &dyn crate::Resolver,
        path: &str,
    ) -> Result<bool, Error> {
        self.hashed_load(resolver, |fdm, r| {
            let document = crate::xml::DocumentLoader::new(r, ".").load_document(path)?;
            fdm.load_initial_conditions_document(&document)
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
