// SPDX-License-Identifier: LGPL-2.1-or-later
// FGFDMExec: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// FGModelFunctions: Copyright (C) 2010 Jon S. Berndt (jon@jsbsim.org).
// FGPropertyReader: Copyright (C) 2014 Bertrand Coconnier.
// Derived from JSBSim src/FGFDMExec.cpp (LoadModel, LoadPlanet),
// src/models/FGModel.cpp (Upload), src/math/FGModelFunctions.cpp and
// src/input_output/FGPropertyReader.cpp.
//
//! Loading an aircraft configuration into the models.

use super::order::Model;
use super::{Binder, Fdm};
use crate::functions::{FnId, FunctionSet};
use crate::properties::{Attribute, NodeMap, PropertyTree};
use crate::xml::{DocumentLoader, Element, IncludeKind};
use crate::{Error, LoadOptions, Resolver};

/// What a model's `Load` may touch besides the model itself: the tree
/// (through the binder), the functions and the options.
pub struct LoadContext<'a> {
    /// Functions and tables.
    pub functions: &'a mut FunctionSet,
    /// Tied-property bindings and the property tree.
    pub binder: Binder<'a>,
    /// Load options.
    pub options: LoadOptions,
    /// The initial condition's position (`GetIC()->GetPosition()`), while
    /// FCS channels load.
    pub ic_position: Option<crate::math::Location>,
}

impl LoadContext<'_> {
    /// The property tree.
    pub fn tree(&mut self) -> &mut PropertyTree {
        self.binder.tree
    }
}

/// `FGModelFunctions` plus `FGPropertyReader`: a model's `<property>`
/// declarations and its pre- and post-functions.
#[derive(Debug, Clone, Default)]
pub struct ModelFunctions {
    /// Functions run (and cached) before the model.
    pub pre: Vec<FnId>,
    /// Functions run (and cached) after the model.
    pub post: Vec<FnId>,
    /// Declared properties and the value `InitModel` restores.
    pub local_properties: NodeMap<f64>,
}

impl Fdm {
    /// `FGPropertyReader::Load`: `<property value="v">path</property>`
    /// declarations. Existing nodes are overwritten only with `overwrite`.
    fn read_properties(&mut self, el: &Element, overwrite: bool, slot: Model) -> Result<(), Error> {
        for p in el.find_elements("property") {
            let has_value = !p.attribute("value").is_empty();
            let value = if has_value {
                p.attribute_as_number("value")?
            } else {
                0.0
            };
            let name = p.data_line(0);
            let node = match self.env.tree.has_node(name)? {
                true => {
                    let Some(node) = self.env.tree.get(name)? else {
                        continue;
                    };
                    if !overwrite {
                        if has_value {
                            tracing::warn!(
                                "{}: Property {name} is already defined. Its value will not be overridden.",
                                p.location()
                            );
                        }
                        continue;
                    }
                    self.write_node(node, value)?;
                    node
                }
                false => {
                    let node = self.env.tree.get_or_create(self.env.tree.root(), name)?;
                    self.write_node(node, value)?;
                    node
                }
            };
            self.model_functions[slot]
                .local_properties
                .insert(node, value);
            if p.attribute("persistent") == "true" {
                self.env.tree.set_attribute(node, Attribute::PRESERVE, true);
            }
        }
        Ok(())
    }

    /// `FGModelFunctions::Load` and `PreLoad`: declarations, then the
    /// pre-functions and templates.
    pub(super) fn load_model_functions(
        &mut self,
        el: &Element,
        prefix: &str,
        slot: Model,
    ) -> Result<(), Error> {
        self.read_properties(el, false, slot)?;
        self.pre_load(el, prefix, slot)
    }

    /// `FGModelFunctions::PreLoad`: the pre-functions and templates.
    pub(super) fn pre_load(
        &mut self,
        el: &Element,
        prefix: &str,
        slot: Model,
    ) -> Result<(), Error> {
        for f in el.find_elements("function") {
            match f.attribute("type") {
                "" | "pre" => {
                    let id = self
                        .env
                        .functions
                        .load_function(&mut self.env.tree, f, prefix)?;
                    self.model_functions[slot].pre.push(id);
                }
                "template" => {
                    let name = f.attribute("name").to_owned();
                    self.env
                        .functions
                        .add_template(&mut self.env.tree, &name, f)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// `FGModelFunctions::PostLoad`.
    pub(super) fn post_load(
        &mut self,
        el: &Element,
        prefix: &str,
        slot: Model,
    ) -> Result<(), Error> {
        for f in el
            .find_elements("function")
            .filter(|f| f.attribute("type") == "post")
        {
            let id = self
                .env
                .functions
                .load_function(&mut self.env.tree, f, prefix)?;
            self.model_functions[slot].post.push(id);
        }
        Ok(())
    }

    /// `FGModel::Upload(el, true)`: resolves a `file` include, loads the
    /// model functions, and merges the included document into `el`.
    pub(super) fn upload(
        &mut self,
        loader: &mut DocumentLoader<'_>,
        el: &mut Element,
        kind: IncludeKind,
        slot: Model,
    ) -> Result<(), Error> {
        let document = loader.open(el, kind)?;
        if let Some(doc) = &document
            && doc.name() != el.name()
        {
            return Err(Error::Model {
                message: format!(
                    "{}: Read model '{}' while expecting model '{}'",
                    el.location(),
                    doc.name(),
                    el.name()
                ),
            });
        }
        match document {
            None => self.load_model_functions(el, "", slot),
            Some(doc) => {
                self.load_model_functions(&doc, "", slot)?;
                el.merge_attributes(&doc);
                self.read_properties(el, true, slot)?;
                for child in doc.children() {
                    el.add_child(child.clone());
                }
                Ok(())
            }
        }
    }

    /// `FGFDMExec::LoadModel(model)` for `aircraft_path/model/model.xml`.
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
    /// [`Error::Model`] too when this `Fdm` has loaded a model already,
    /// or begun to: each `Fdm` holds one aircraft, where JSBSim would
    /// deallocate the first. A failed load is not rolled back: start again
    /// from a new `Fdm`. A model document that is not found begins no
    /// load, so another may follow.
    pub fn load_model(
        &mut self,
        resolver: &dyn Resolver,
        aircraft_path: &str,
        model: &str,
    ) -> Result<(), Error> {
        self.hashed_load(resolver, |fdm, r| {
            fdm.load_model_from(r, aircraft_path, model)
        })
    }

    /// `FGFDMExec::LoadModel(model, false)` for the model file at `path`,
    /// such as `models/Rascal/Rascal110-JSBSim` in PX4's `jsbsim_bridge`
    /// tree, where a model is not in a directory of its own name. As
    /// JSBSim appends `.xml` to the model name, `.xml` is added unless
    /// `path` ends in it. The file's directory is the aircraft directory
    /// its includes are found in.
    ///
    /// # Errors
    ///
    /// Those of [`load_model`](Self::load_model).
    pub fn load_model_file(&mut self, resolver: &dyn Resolver, path: &str) -> Result<(), Error> {
        let file = crate::xml::xml_file_path(path);
        let dir = match file.rfind('/') {
            Some(0) => "/",
            Some(at) => &file[..at],
            None => ".",
        };
        self.hashed_load(resolver, |fdm, r| fdm.load_model_document(r, dir, &file))
    }

    /// [`load_model`](Self::load_model) within another load.
    pub(super) fn load_model_from(
        &mut self,
        resolver: &dyn Resolver,
        aircraft_path: &str,
        model: &str,
    ) -> Result<(), Error> {
        let dir = format!("{aircraft_path}/{model}");
        self.load_model_document(resolver, &dir, &format!("{dir}/{model}.xml"))
    }

    /// Loads the model document at `path` with `dir` as `FullAircraftPath`.
    fn load_model_document(
        &mut self,
        resolver: &dyn Resolver,
        dir: &str,
        path: &str,
    ) -> Result<(), Error> {
        self.refuse_second_model()?;
        let mut loader = self.document_loader(resolver, dir);
        let document = loader.load_document(path)?;
        dir.clone_into(&mut self.full_aircraft_path);
        self.model_loaded = true;
        self.load_document(&mut loader, document)
    }

    /// [`Error::Model`] once a model has begun loading.
    pub(super) fn refuse_second_model(&self) -> Result<(), Error> {
        match self.model_loaded {
            true => Err(Error::Model {
                message: format!(
                    "A model is already loaded from {}: load another into a new Fdm.",
                    self.full_aircraft_path
                ),
            }),
            false => Ok(()),
        }
    }

    /// A loader for the aircraft in `dir` with this executive's systems
    /// and engine paths.
    pub(super) fn document_loader<'r>(
        &self,
        resolver: &'r dyn Resolver,
        dir: &str,
    ) -> DocumentLoader<'r> {
        DocumentLoader::new(resolver, dir)
            .with_systems_dir(self.systems_path.clone())
            .with_engine_dir(self.engine_path.clone())
    }

    fn load_document(
        &mut self,
        loader: &mut DocumentLoader<'_>,
        document: Element,
    ) -> Result<(), Error> {
        self.models.aircraft.name = document.attribute("name").to_owned();
        if let Some(mut planet) = document.find_element("planet").cloned() {
            self.load_planet(loader, &mut planet)?;
        }
        let take = |name: &str| document.find_element(name).cloned();
        let Some(mut metrics) = take("metrics") else {
            return Err(Error::Model {
                message: "No metrics element was found in the aircraft config file.".to_owned(),
            });
        };
        self.upload(loader, &mut metrics, IncludeKind::Model, Model::Aircraft)?;
        self.models.aircraft.load(&metrics)?;
        self.post_load(&metrics, "", Model::Aircraft)?;
        let Some(mut mass) = take("mass_balance") else {
            return Err(Error::Model {
                message: "No mass_balance element was found in the aircraft config file."
                    .to_owned(),
            });
        };
        self.upload(loader, &mut mass, IncludeKind::Model, Model::MassBalance)?;
        self.load_mass_balance(&mass)?;
        self.post_load(&mass, "", Model::MassBalance)?;
        let Some(mut ground) = take("ground_reactions") else {
            return Err(Error::Model {
                message: "No ground_reactions element was found in the aircraft config file."
                    .to_owned(),
            });
        };
        self.upload(
            loader,
            &mut ground,
            IncludeKind::Model,
            Model::GroundReactions,
        )?;
        self.load_ground_reactions(&ground)?;
        if let Some(mut external) = take("external_reactions") {
            self.upload(
                loader,
                &mut external,
                IncludeKind::Model,
                Model::ExternalReactions,
            )?;
            self.load_external_reactions(&external)?;
            self.post_load(&external, "", Model::ExternalReactions)?;
        }
        self.load_systems_and_rest(loader, &document)
    }

    /// `LoadModel` from `<buoyant_forces>` on.
    fn load_systems_and_rest(
        &mut self,
        loader: &mut DocumentLoader<'_>,
        document: &Element,
    ) -> Result<(), Error> {
        if let Some(mut buoyant) = document.find_element("buoyant_forces").cloned() {
            self.load_buoyant_forces(loader, &mut buoyant)?;
        }
        if let Some(mut propulsion) = document.find_element("propulsion").cloned() {
            self.load_propulsion(loader, &mut propulsion)?;
        }
        let systems: Vec<Element> = document
            .children()
            .iter()
            .filter(|c| matches!(c.name(), "system"))
            .cloned()
            .collect();
        for mut system in systems {
            self.load_fcs(loader, &mut system, IncludeKind::System)?;
        }
        for name in ["autopilot", "flight_control"] {
            if let Some(mut el) = document.find_element(name).cloned() {
                self.load_fcs(loader, &mut el, IncludeKind::Model)?;
            }
        }
        if let Some(mut aero) = document.find_element("aerodynamics").cloned() {
            self.upload(loader, &mut aero, IncludeKind::Model, Model::Aerodynamics)?;
            self.load_aerodynamics(&aero)?;
            self.post_load(&aero, "", Model::Aerodynamics)?;
        }
        self.io_directives(document, loader, None)?;
        unsupported_if_present(document, "child")?;
        self.load_model_constants();
        self.load_all_inputs();
        Ok(())
    }

    /// The `<input>` and then the `<output>` children of `document`.
    /// They describe sockets and files, which the core does not open; the
    /// harness logs properties itself. Everything else they do is loaded
    /// and run as in JSBSim, strict or not: each `<input>` makes an input
    /// type with its declarations and functions, each `<output>` an output
    /// type and that type's properties, as `FGOutput::Load` does. An
    /// output's `file` is looked for in `include_dir` (the script's
    /// directory, for a script's outputs), then in the aircraft directory.
    pub(super) fn io_directives(
        &mut self,
        document: &Element,
        loader: &mut DocumentLoader<'_>,
        include_dir: Option<&str>,
    ) -> Result<(), Error> {
        for el in document.find_elements("input") {
            self.load_input(el, loader)?;
        }
        for el in document.find_elements("output") {
            tracing::info!(
                "{}: <output> writes nothing: the core does no file or socket I/O",
                el.location()
            );
            self.load_output(el, loader, include_dir)?;
        }
        Ok(())
    }

    /// `FGOutput::Load(element)`: a `file` include is merged into the
    /// element (`FGModel::Upload`), its attributes and children added. The
    /// type's functions, the included file's among them, load as
    /// `PreLoad` and `PostLoad` load a model's.
    fn load_output(
        &mut self,
        el: &Element,
        loader: &DocumentLoader<'_>,
        include_dir: Option<&str>,
    ) -> Result<(), Error> {
        let merged;
        let el = if el.has_attribute("file") {
            let file = el.attribute("file");
            let path = loader
                .find_output_include(include_dir, file)
                .ok_or_else(|| crate::xml::XmlError::IncludeNotFound {
                    file: file.to_owned(),
                    at: el.location(),
                })?;
            let doc = loader.load_document(&path)?;
            if doc.name() != el.name() {
                return Err(Error::Model {
                    message: format!(
                        "{}: Read model '{}' while expecting model '{}'",
                        el.location(),
                        doc.name(),
                        el.name()
                    ),
                });
            }
            let mut copy = el.clone();
            copy.merge_attributes(&doc);
            for child in doc.children() {
                copy.add_child(child.clone());
            }
            merged = copy;
            &merged
        } else {
            el
        };
        let dt = self.models.exec.dt;
        self.models.output.load(el, &mut self.env.binder(), dt)?;
        let (mut pre, mut post) = (Vec::new(), Vec::new());
        for f in el.find_elements("function") {
            let kind = f.attribute("type");
            if kind == "template" {
                let name = f.attribute("name").to_owned();
                self.env
                    .functions
                    .add_template(&mut self.env.tree, &name, f)?;
                continue;
            }
            let functions = &mut self.env.functions;
            match kind {
                "" | "pre" => pre.push(functions.load_function(&mut self.env.tree, f, "")?),
                "post" => post.push(functions.load_function(&mut self.env.tree, f, "")?),
                _ => {}
            }
        }
        if let Some(t) = self.models.output.types.last_mut() {
            t.pre = pre;
            t.post = post;
        }
        Ok(())
    }
}

fn unsupported_if_present(document: &Element, name: &str) -> Result<(), Error> {
    match document.find_element(name) {
        Some(el) => Err(Error::Unsupported {
            construct: format!("<{name}>"),
            path: el.location().to_string(),
        }),
        None => Ok(()),
    }
}
