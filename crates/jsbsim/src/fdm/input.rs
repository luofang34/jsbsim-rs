// SPDX-License-Identifier: LGPL-2.1-or-later
// FGInput: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// FGInputType, FGInputSocket, FGUDPInputSocket: Copyright (C) 2013
// Bertrand Coconnier.
// Derived from JSBSim src/models/FGInput.cpp (Load, InitModel, Run),
// src/input_output/FGInputType.cpp (Load, InitModel, Run),
// src/input_output/FGInputSocket.cpp (Load) and
// src/input_output/FGUDPInputSocket.cpp (Load).
//
//! Loading and running the `<input>` directives, without their sockets.

use super::Fdm;
use super::order::Model;
use crate::Error;
use crate::models::input::{InputType, udp_rate};
use crate::properties::Attribute;
use crate::xml::{DocumentLoader, Element, IncludeKind};

#[cfg(test)]
mod tests;

/// The input classes `FGInput::Load` makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// `FGInputSocket`, a TCP command server.
    Socket,
    /// `FGUDPInputSocket`, a UDP property feed.
    Udp,
}

impl Fdm {
    /// `FGInput::Load(el)`. A `file` include stands in for `el` entirely,
    /// its attributes and children both, as `FGModelLoader::Open` returns
    /// it. Its functions load twice, as in JSBSim: once as `FGInput`'s own,
    /// which never run, and once as the new type's.
    pub(super) fn load_input(
        &mut self,
        el: &Element,
        loader: &mut DocumentLoader<'_>,
    ) -> Result<(), Error> {
        let mut el = el.clone();
        let opened = loader.open(&mut el, IncludeKind::Model)?;
        let element = opened.as_ref().unwrap_or(&el);
        // `FGInputType::Load` would open a nested include and, on a root
        // of another name, stop without its post-functions.
        if opened.is_some() && element.has_attribute("file") {
            return Err(Error::Unsupported {
                construct: "<input file=...> whose file is itself an include".to_owned(),
                path: element.location().to_string(),
            });
        }
        self.load_dead_functions(element, true)?;
        let kind = element.attribute("type");
        let class = match kind.to_ascii_uppercase().as_str() {
            "" | "SOCKET" => Class::Socket,
            "QTJSBSIM" => Class::Udp,
            upper => {
                if upper != "NONE" {
                    tracing::error!(
                        "{}: Unknown type of input specified in config file",
                        element.location()
                    );
                }
                return Err(Error::UnknownInputType {
                    kind: kind.to_owned(),
                    path: element.location().to_string(),
                });
            }
        };
        let mut input = InputType::default();
        input.base.enabled = self.models.input.base.enabled;
        // `FGInputType::Load`: `Upload(el, true)` on an element without an
        // include, then `PostLoad`. The type's functions are gathered in
        // the input slot and moved out, leaving `FGInput`'s own in place.
        let own = std::mem::take(&mut self.model_functions[Model::Input]);
        let loaded = self
            .load_model_functions(element, "", Model::Input)
            .and_then(|()| self.post_load(element, "", Model::Input));
        input.functions = std::mem::replace(&mut self.model_functions[Model::Input], own);
        loaded?;
        self.socket_attributes(element, class, &mut input);
        self.load_dead_functions(element, false)?;
        self.models.input.types.push(input);
        Ok(())
    }

    /// `FGInput`'s own `PreLoad` (`pre`) or `PostLoad` of `el`: copies of
    /// the type's functions that nothing runs, though a named one binds its
    /// property first, so the type's copy then fails to bind. They load
    /// leniently: the pre-functions load before the type declares its
    /// properties, so a `copyto` naming one of them is logged and ignored
    /// here though the type's own copy honours it.
    fn load_dead_functions(&mut self, el: &Element, pre: bool) -> Result<(), Error> {
        let options = self
            .env
            .functions
            .replace_options(crate::LoadOptions::default());
        let loaded = if pre {
            self.pre_load(el, "", Model::Input)
        } else {
            self.post_load(el, "", Model::Input)
        };
        self.env.functions.replace_options(options);
        loaded
    }

    /// `FGInputSocket::Load` and `FGUDPInputSocket::Load` past the type's
    /// own: the port, the blocking action and the UDP rate. Their `false`
    /// returns are ignored by `FGInput::Load`, so each only logs.
    fn socket_attributes(&self, el: &Element, class: Class, input: &mut InputType) {
        tracing::info!(
            "{}: <input> opens no socket: the core does no socket I/O",
            el.location()
        );
        let port = crate::c_numeric::atoi(el.attribute("port"));
        if port == 0 {
            tracing::error!("{}: No port assigned in input element", el.location());
            return;
        }
        if el
            .attribute("action")
            .eq_ignore_ascii_case("BLOCKING_INPUT")
        {
            tracing::warn!(
                "{}: <input action=\"BLOCKING_INPUT\"> does not block: the core opens no socket",
                el.location()
            );
        }
        if class == Class::Udp {
            let rate = crate::c_numeric::atoi(el.attribute("rate"));
            input.base.rate = udp_rate(rate, self.models.exec.dt);
        }
    }

    /// `FGInput::InitModel` from `RunIC`: each type's frame counter, and
    /// its declared properties back to their initial values unless
    /// persistent.
    pub(super) fn init_input(&mut self) -> Result<(), Error> {
        self.models.input.base.init();
        for i in 0..self.models.input.types.len() {
            let Some(t) = self.models.input.types.get_mut(i) else {
                continue;
            };
            t.base.init();
            let slots = t.functions.local_properties.slot_count();
            // A write may reach a setter, so each entry is copied out by
            // slot rather than borrowed across it.
            for slot in 0..slots {
                let entry = self.models.input.types.get(i).and_then(|t| {
                    t.functions
                        .local_properties
                        .entry(slot)
                        .map(|(node, &value)| (node, value))
                });
                let Some((node, value)) = entry else {
                    continue;
                };
                if !self.env.tree.attribute(node, Attribute::PRESERVE) {
                    self.write_node(node, value)?;
                }
            }
        }
        Ok(())
    }

    /// `FGInput::Run`: nothing during a trim or while the input is
    /// disabled; otherwise each type due this frame caches its pre- and
    /// post-functions around the socket read the port does not make.
    pub(super) fn run_input(&mut self) -> Result<(), Error> {
        let trim_status = self.models.exec.trim_status;
        if !self.models.input.runs(trim_status) {
            return Ok(());
        }
        for i in 0..self.models.input.types.len() {
            let Some(t) = self.models.input.types.get_mut(i) else {
                continue;
            };
            if t.base.skip() {
                continue;
            }
            for pre in [true, false] {
                let mut k = 0;
                while let Some(id) = self.models.input.types.get(i).and_then(|t| {
                    let ids = if pre {
                        &t.functions.pre
                    } else {
                        &t.functions.post
                    };
                    ids.get(k).copied()
                }) {
                    self.env.cache_value(&self.models, id)?;
                    k += 1;
                }
            }
        }
        Ok(())
    }

    /// `FGFDMExec::EnableInput`.
    #[cfg(test)]
    pub(crate) fn enable_input(&mut self) {
        self.models.input.enable();
    }

    /// `FGFDMExec::DisableInput`.
    #[cfg(test)]
    pub(crate) fn disable_input(&mut self) {
        self.models.input.disable();
    }
}
