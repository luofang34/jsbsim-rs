// SPDX-License-Identifier: LGPL-2.1-or-later
// FGOutput: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// FGOutputType: Copyright (C) 2011 Bertrand Coconnier.
// Derived from JSBSim src/models/FGOutput.cpp (Load, Run) and
// src/input_output/FGOutputType.cpp (Load, Run).
//
//! Loading and running the `<output>` directives, without their files and
//! sockets.

use super::Fdm;
use crate::Error;
use crate::xml::{DocumentLoader, Element};

impl Fdm {
    /// `FGOutput::Load(element)`: a `file` include is merged into the
    /// element (`FGModel::Upload`), its attributes and children added. The
    /// type's functions, the included file's among them, load as
    /// `PreLoad` and `PostLoad` load a model's.
    pub(super) fn load_output(
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

    /// `FGOutput::Run`: each output type due to print caches its pre- and
    /// post-functions around the print the port does not make.
    pub(super) fn run_output(&mut self, holding: bool) -> Result<(), Error> {
        let trim_status = self.models.exec.trim_status;
        if !self.models.output.runs(holding, trim_status) {
            return Ok(());
        }
        for i in 0..self.models.output.types.len() {
            let Some(t) = self.models.output.types.get_mut(i) else {
                continue;
            };
            if t.base.skip() {
                continue;
            }
            for pre in [true, false] {
                let mut k = 0;
                while let Some(id) = self.models.output.types.get(i).and_then(|t| {
                    let ids = if pre { &t.pre } else { &t.post };
                    ids.get(k).copied()
                }) {
                    self.env.cache_value(&self.models, id)?;
                    k += 1;
                }
            }
        }
        Ok(())
    }
}
