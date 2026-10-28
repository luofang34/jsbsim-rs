// SPDX-License-Identifier: LGPL-2.1-or-later
// FGFDMExec: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// FGModelFunctions: Copyright (C) 2010 Jon S. Berndt (jon@jsbsim.org).
// FGPropertyReader: Copyright (C) 2014 Bertrand Coconnier.
// Derived from JSBSim src/FGFDMExec.cpp (LoadModel, LoadPlanet),
// src/models/FGModel.cpp (Upload), src/math/FGModelFunctions.cpp and
// src/input_output/FGPropertyReader.cpp.
//
//! Loading an aircraft configuration into the models.

use std::collections::BTreeMap;

use super::order::Model;
use super::{Binder, Fdm};
use crate::functions::{FnId, FunctionSet};
use crate::properties::{Attribute, NodeId, PropertyTree};
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
    pub local_properties: BTreeMap<NodeId, f64>,
}

impl Fdm {
    pub(super) fn context(&mut self) -> LoadContext<'_> {
        LoadContext {
            functions: &mut self.functions,
            binder: Binder {
                tree: &mut self.tree,
                bindings: &mut self.bindings,
            },
            options: self.options,
        }
    }

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
            let node = match self.tree.has_node(name)? {
                true => {
                    let Some(node) = self.tree.get(name)? else {
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
                    let node = self.tree.get_or_create(self.tree.root(), name)?;
                    self.write_node(node, value)?;
                    node
                }
            };
            self.model_functions[slot as usize]
                .local_properties
                .insert(node, value);
            if p.attribute("persistent") == "true" {
                self.tree.set_attribute(node, Attribute::PRESERVE, true);
            }
        }
        Ok(())
    }

    /// `FGModelFunctions::Load` and `PreLoad`: declarations, then the
    /// pre-functions and templates.
    fn load_model_functions(
        &mut self,
        el: &Element,
        prefix: &str,
        slot: Model,
    ) -> Result<(), Error> {
        self.read_properties(el, false, slot)?;
        for f in el.find_elements("function") {
            match f.attribute("type") {
                "" | "pre" => {
                    let id = self.functions.load_function(&mut self.tree, f, prefix)?;
                    self.model_functions[slot as usize].pre.push(id);
                }
                "template" => {
                    let name = f.attribute("name").to_owned();
                    self.functions.add_template(&mut self.tree, &name, f)?;
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
            let id = self.functions.load_function(&mut self.tree, f, prefix)?;
            self.model_functions[slot as usize].post.push(id);
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
        let mut loader = DocumentLoader::new(resolver, dir.clone());
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
        if let Some(propulsion) = document.find_element("propulsion") {
            refuse_children(propulsion)?;
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
        for name in ["input", "output"] {
            for el in document.find_elements(name) {
                self.io_directive(el)?;
            }
        }
        unsupported_if_present(document, "child")?;
        self.load_model_constants();
        self.load_all_inputs();
        Ok(())
    }

    /// `<input>` and `<output>` describe sockets and files, which the core
    /// does not open; the harness logs properties itself.
    pub(super) fn io_directive(&self, el: &Element) -> Result<(), Error> {
        if self.options.strict {
            return Err(Error::Unsupported {
                construct: format!("<{}> (file and socket I/O)", el.name()),
                path: el.location().to_string(),
            });
        }
        tracing::warn!(
            "{}: <{}> is ignored: the core does no file or socket I/O",
            el.location(),
            el.name()
        );
        Ok(())
    }

    /// `LoadPlanet(element)`.
    fn load_planet(&mut self, el: &Element) -> Result<(), Error> {
        self.models.inertial.load(el)?;
        self.load_planet_constants();
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

/// A model that is not ported yet may only be present empty, or hold
/// nothing but `<documentation>`, which JSBSim ignores.
pub(super) fn refuse_children(el: &Element) -> Result<(), Error> {
    match el.children().iter().find(|c| c.name() != "documentation") {
        Some(child) => Err(Error::Unsupported {
            construct: format!("<{}> in <{}>", child.name(), el.name()),
            path: child.location().to_string(),
        }),
        None => Ok(()),
    }
}
