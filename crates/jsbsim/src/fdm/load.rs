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
    pub fn load_model(
        &mut self,
        resolver: &dyn Resolver,
        aircraft_path: &str,
        model: &str,
    ) -> Result<(), Error> {
        let dir = format!("{aircraft_path}/{model}");
        let mut loader =
            DocumentLoader::new(resolver, dir.clone()).with_systems_dir(self.systems_path.clone());
        let document = loader.load_document(&format!("{dir}/{model}.xml"))?;
        self.load_document(&mut loader, document)
    }

    fn load_document(
        &mut self,
        loader: &mut DocumentLoader<'_>,
        document: Element,
    ) -> Result<(), Error> {
        if let Some(planet) = document.find_element("planet").cloned() {
            self.load_planet(&planet)?;
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
        unsupported_if_present(document, "buoyant_forces")?;
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
    /// harness logs properties itself. Each `<output>` still makes its
    /// output type and that type's properties, as `FGOutput::Load` does;
    /// an output's `file` is looked for in `include_dir` (the script's
    /// directory, for a script's outputs), then in the aircraft directory.
    pub(super) fn io_directives(
        &mut self,
        document: &Element,
        loader: &DocumentLoader<'_>,
        include_dir: Option<&str>,
    ) -> Result<(), Error> {
        for name in ["input", "output"] {
            for el in document.find_elements(name) {
                if self.options.strict {
                    return Err(Error::Unsupported {
                        construct: format!("<{name}> (file and socket I/O)"),
                        path: el.location().to_string(),
                    });
                }
                tracing::warn!(
                    "{}: <{name}> writes nothing: the core does no file or socket I/O",
                    el.location()
                );
                if name == "output" {
                    self.load_output(el, loader, include_dir)?;
                }
            }
        }
        Ok(())
    }

    /// `FGOutput::Load(element)`: a `file` include is merged into the
    /// element (`FGModel::Upload`), its attributes and children added. The
    /// output's own functions, and the included file's, are not loaded:
    /// JSBSim ties each named one to a property whose value it caches only
    /// when the type prints, which the port never does, so the port leaves
    /// those properties out; templates there only serve the output's
    /// `apply` conversions.
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
        if el.find_element("function").is_some() {
            tracing::warn!(
                "{}: the functions of an <output> are not loaded",
                el.location()
            );
        }
        let dt = self.models.exec.dt;
        self.models.output.load(el, &mut self.env.binder(), dt)
    }

    /// `LoadPlanet(element)`.
    fn load_planet(&mut self, el: &Element) -> Result<(), Error> {
        self.models.inertial.load(el)?;
        self.load_planet_constants();
        self.models.ic.initialize_ic(&self.models.inertial);
        self.initialize_models()?;
        if let Some(atm) = el.find_element("atmosphere")
            && atm.has_attribute("model")
        {
            let model = atm.attribute("model").to_ascii_lowercase();
            if model == "msis" || model == "mars" {
                return Err(Error::Unsupported {
                    construct: format!("<atmosphere model=\"{model}\">"),
                    path: atm.location().to_string(),
                });
            }
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
