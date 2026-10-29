// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/input_output/FGScript.cpp (LoadScript,
// ResetEvents, RunScript) and src/FGFDMExec.cpp (LoadScript).
//
//! Loading and running scripts.

use super::Fdm;
use crate::functions::{Condition, Parameter, PropertyValue};
use crate::script::{Event, Script, Set, SetAction, SetType};
use crate::xml::{DocumentLoader, Element};
use crate::{Error, Resolver};

fn script_error(message: impl Into<String>) -> Error {
    Error::Model {
        message: message.into(),
    }
}

fn parse_set(el: &Element, fdm: &mut Fdm) -> Result<Set, Error> {
    let name = el.attribute("name").to_owned();
    let node = if fdm.env.tree.has_node(&name)? {
        fdm.env.tree.get(&name)?
    } else {
        None
    };
    let (value, function) = if !el.attribute("value").is_empty() {
        (el.attribute_as_number("value")?, None)
    } else if let Some(f) = el.find_element("function") {
        (
            0.0,
            Some(fdm.env.functions.load_function(&mut fdm.env.tree, f, "")?),
        )
    } else {
        (0.0, None)
    };
    let lower = |a: &str| el.attribute(a).to_ascii_lowercase();
    let kind = match lower("type") {
        t if t.contains("delta") => SetType::Delta,
        t if t.contains("bool") => SetType::Bool,
        _ => SetType::Value,
    };
    let action = match lower("action") {
        a if a.contains("ramp") => SetAction::Ramp,
        a if a.contains("step") => SetAction::Step,
        a if a.contains("exp") => SetAction::Exp,
        _ => SetAction::Step,
    };
    let tc = if el.attribute("tc").is_empty() {
        1.0
    } else {
        el.attribute_as_number("tc")?
    };
    Ok(Set {
        name,
        node,
        function,
        value,
        original: 0.0,
        new_value: 0.0,
        span: 0.0,
        kind,
        action,
        tc,
        transiting: false,
    })
}

fn parse_event(el: &Element, fdm: &mut Fdm) -> Result<Event, Error> {
    let Some(condition_el) = el.find_element("condition") else {
        return Err(script_error(format!(
            "{}: No condition specified in script event {}",
            el.location(),
            el.attribute("name")
        )));
    };
    let condition = Condition::load(condition_el, &fdm.env.tree)?;
    let delay = if el.find_element("delay").is_some() {
        el.find_element_value_as_number("delay")?
    } else {
        0.0
    };
    let mut event = Event {
        name: el.attribute("name").to_owned(),
        persistent: el.attribute("persistent") == "true",
        continuous: el.attribute("continuous") == "true",
        condition,
        delay,
        notify: false,
        description: String::new(),
        notify_properties: Vec::new(),
        sets: Vec::new(),
        triggered: false,
        notified: false,
        start_time: 0.0,
    };
    if let Some(notify) = el.find_element("notify") {
        event.notify = true;
        event.description = notify.find_element_value("description").to_owned();
        for p in notify.find_elements("property") {
            let name = p.data_line(0);
            let value = PropertyValue::new(name, &fdm.env.tree, p.location())?;
            let param = if p.has_attribute("apply") {
                match fdm.env.functions.template(p.attribute("apply")) {
                    Some(template) => Parameter::Template {
                        template,
                        property: value,
                    },
                    None => {
                        tracing::error!(
                            "{}: No function by the name {} has been defined.",
                            p.location(),
                            p.attribute("apply")
                        );
                        continue;
                    }
                }
            } else {
                Parameter::Property(value)
            };
            let caption = match p.attribute("caption") {
                "" => name.to_owned(),
                c => c.to_owned(),
            };
            event.notify_properties.push((param, caption));
        }
    }
    for set in el.find_elements("set") {
        event.sets.push(parse_set(set, fdm)?);
    }
    Ok(event)
}

impl Fdm {
    /// `FGFDMExec::LoadScript(script)`: the run span and time step, the
    /// aircraft and initial conditions, the declarations and the events.
    pub fn load_script(&mut self, resolver: &dyn Resolver, script_path: &str) -> Result<(), Error> {
        let loader = DocumentLoader::new(resolver, self.aircraft_path.clone());
        let document = loader.load_document(script_path)?;
        if document.name() != "runscript" {
            return Err(script_error(format!(
                "File: {script_path} is not a script file"
            )));
        }
        let Some(run) = document.find_element("run") else {
            return Err(script_error("No \"run\" element found in script."));
        };
        let mut script = Script {
            name: document.attribute("name").to_owned(),
            ..Script::default()
        };
        script.start_time = if run.has_attribute("start") {
            run.attribute_as_number("start")?
        } else {
            0.0
        };
        self.set_sim_time(script.start_time);
        if !run.has_attribute("end") {
            return Err(script_error(
                "An end time (duration) for the script must be specified in the script <run> element.",
            ));
        }
        script.end_time = run.attribute_as_number("end")?;
        let dt = run.attribute_as_number("dt")?;
        self.set_dt(dt);
        script.end_time += 0.99 * self.models.exec.dt;
        let Some(use_el) = document.find_element("use") else {
            return Err(script_error("No \"use\" directives in the script file."));
        };
        let aircraft = use_el.attribute("aircraft");
        if aircraft.is_empty() {
            return Err(script_error("Aircraft must be specified in use element."));
        }
        let aircraft_path = self.aircraft_path.clone();
        self.load_model(resolver, &aircraft_path, aircraft)?;
        let initialize = use_el.attribute("initialize");
        if initialize.is_empty() {
            return Err(script_error(
                "Initialization file must be specified in use element.",
            ));
        }
        if !self.load_initial_conditions(
            resolver,
            &format!("{aircraft_path}/{aircraft}"),
            initialize,
        )? {
            return Err(script_error("Initialization unsuccessful"));
        }
        let script_dir = match script_path.rfind('/') {
            Some(at) => &script_path[..at],
            None => ".",
        };
        // An output's include falls back on the full aircraft path.
        let mut outputs = DocumentLoader::new(resolver, format!("{aircraft_path}/{aircraft}"));
        self.io_directives(&document, &mut outputs, Some(script_dir))?;
        script.local_properties = self.declare_script_properties(run)?;
        for event_el in run.find_elements("event") {
            script.events.push(parse_event(event_el, self)?);
        }
        self.script = Some(script);
        Ok(())
    }

    fn declare_script_properties(
        &mut self,
        run: &Element,
    ) -> Result<crate::properties::NodeMap<f64>, Error> {
        let mut declared = crate::properties::NodeMap::new();
        for p in run.find_elements("property") {
            let value = if p.attribute("value").is_empty() {
                0.0
            } else {
                p.attribute_as_number("value")?
            };
            let node = self
                .env
                .tree
                .get_or_create(self.env.tree.root(), p.data_line(0))?;
            self.write_node(node, value)?;
            declared.insert(node, value);
            if p.attribute("persistent") == "true" {
                self.env
                    .tree
                    .set_attribute(node, crate::properties::Attribute::PRESERVE, true);
            }
        }
        Ok(declared)
    }

    /// `Setsim_time`.
    pub fn set_sim_time(&mut self, t: f64) {
        self.models.exec.sim_time = t;
        self.models.inertial.set_time(t);
        self.models.atmosphere.sim_time = t;
    }

    /// `FGScript::ResetEvents`. While [`run_script`](Self::run_script)
    /// has the events out, they are reset when it gets them back.
    pub(super) fn reset_events(&mut self) -> Result<(), Error> {
        let Some(script) = self.script.as_mut() else {
            return Ok(());
        };
        let declared = std::mem::take(&mut script.local_properties);
        let start_time = script.start_time;
        let mut result = Ok(());
        for (node, &value) in declared.iter() {
            if !self
                .env
                .tree
                .attribute(node, crate::properties::Attribute::PRESERVE)
                && let Err(e) = self.write_node(node, value)
            {
                result = Err(e);
                break;
            }
        }
        if let Some(script) = self.script.as_mut() {
            script.local_properties = declared;
            script.events.iter_mut().for_each(Event::reset);
            script.events_reset = true;
        }
        result?;
        self.set_sim_time(start_time);
        Ok(())
    }

    /// Whether `ResetEvents` ran since the last call.
    fn take_events_reset(&mut self) -> bool {
        self.script
            .as_mut()
            .is_some_and(|s| std::mem::take(&mut s.events_reset))
    }

    /// `FGScript::RunScript`: false once the end time is passed. The
    /// events are taken out while they run, since their sets write
    /// properties whose setters reach the executive.
    pub(super) fn run_script(&mut self) -> Result<bool, Error> {
        let Some(script) = self.script.as_mut() else {
            return Ok(true);
        };
        let mut events = std::mem::take(&mut script.events);
        let end_time = script.end_time;
        script.events_reset = false;
        let result = self.run_events(&mut events, end_time);
        if self.take_events_reset() {
            events.iter_mut().for_each(Event::reset);
        }
        if let Some(script) = self.script.as_mut() {
            script.events = events;
        }
        result
    }

    fn run_events(&mut self, events: &mut [Event], end_time: f64) -> Result<bool, Error> {
        let now = self.models.exec.sim_time;
        if now > end_time {
            return Ok(false);
        }
        for counter in 0..events.len() {
            let Some(event) = events.get_mut(counter) else {
                continue;
            };
            let condition = &event.condition;
            let fired = self.env.eval(&self.models, |ev| condition.evaluate(ev))?;
            if fired {
                if !event.triggered {
                    self.fire(event, now)?;
                }
                event.triggered = true;
            } else if event.persistent || event.continuous {
                event.triggered = false;
                event.notified = false;
            }
            if now >= event.start_time && event.triggered {
                let reset = self.act(event, now)?;
                if reset {
                    // The events after this one see the reset too.
                    events.iter_mut().for_each(Event::reset);
                }
                if let Some(event) = events.get_mut(counter) {
                    self.notify(event, counter, now)?;
                }
            }
        }
        Ok(true)
    }

    fn eval_function(&mut self, id: crate::functions::FnId) -> Result<f64, Error> {
        self.env.eval(&self.models, |ev| ev.function(id))
    }

    fn fire(&mut self, event: &mut Event, now: f64) -> Result<(), Error> {
        for set in &mut event.sets {
            let node = match set.node {
                Some(n) => n,
                None => {
                    let n = if self.env.tree.has_node(&set.name)? {
                        self.env.tree.get(&set.name)?
                    } else {
                        None
                    };
                    let n = n.ok_or_else(|| {
                        script_error(format!("No property, \"{}\" is defined.", set.name))
                    })?;
                    set.node = Some(n);
                    n
                }
            };
            set.original = self.read_node(node)?;
            if let Some(f) = set.function {
                set.value = self.eval_function(f)?;
            }
            set.new_value = match set.kind {
                SetType::Value | SetType::Bool => set.value,
                SetType::Delta => set.original + set.value,
            };
            event.start_time = now + event.delay;
            set.span = set.new_value - set.original;
            set.transiting = true;
        }
        Ok(())
    }

    /// The event's transiting sets, written; true when one of them reset
    /// the events, which this event then already shows.
    fn act(&mut self, event: &mut Event, now: f64) -> Result<bool, Error> {
        let mut reset = false;
        for i in 0..event.sets.len() {
            let Some(set) = event.sets.get_mut(i) else {
                continue;
            };
            if !set.transiting {
                continue;
            }
            let time_span = now - event.start_time;
            let value = match set.action {
                SetAction::Ramp => {
                    if time_span <= set.tc {
                        time_span / set.tc * set.span + set.original
                    } else {
                        if !event.continuous {
                            set.transiting = false;
                        }
                        set.new_value
                    }
                }
                SetAction::Step => {
                    let mut v = set.new_value;
                    if !event.continuous {
                        set.transiting = false;
                    } else if let Some(f) = set.function {
                        v = self.eval_function(f)?;
                    }
                    v
                }
                SetAction::Exp => {
                    (1.0 - crate::elementary::exp(-time_span / set.tc)) * set.span + set.original
                }
            };
            if let Some(node) = set.node {
                self.write_node(node, value)?;
                if self.take_events_reset() {
                    event.reset();
                    reset = true;
                }
            }
        }
        Ok(reset)
    }

    fn notify(&mut self, event: &mut Event, counter: usize, now: f64) -> Result<(), Error> {
        if !event.notify || event.notified {
            return Ok(());
        }
        tracing::info!(
            "Event {} (Event {counter}) executed at time: {now}",
            event.name
        );
        if !event.description.is_empty() {
            tracing::info!("    {}", event.description);
        }
        for (param, caption) in &event.notify_properties {
            let value = self.env.eval(&self.models, |ev| ev.parameter(param))?;
            tracing::info!("    {caption} = {value}");
        }
        event.notified = true;
        Ok(())
    }
}
