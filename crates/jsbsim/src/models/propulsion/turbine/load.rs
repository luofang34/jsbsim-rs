// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2003 David Culp (daveculp@cox.net)
// Derived from JSBSim src/models/propulsion/FGTurbine.cpp (the
// constructor and Load).
//
//! Loading a `<turbine_engine>`.

use super::{Phase, Spool, Turbine, TurbineConfig, TurbineFlags, TurbineLookups, TurbineState};
use crate::Error;
use crate::fdm::EngineLoad;
use crate::functions::FnId;
use crate::models::propulsion::engine::EngineType;
use crate::xml::{Element, atof_locale_c};

/// The engine functions `Load` renames to `propulsion/engine[#]/<name>`.
const FUNCTIONS: [&str; 8] = [
    "IdleThrust",
    "MilThrust",
    "AugThrust",
    "Injection",
    "N1SpoolUp",
    "N1SpoolDown",
    "N2SpoolUp",
    "N2SpoolDown",
];

/// The optional number `name` of `el`.
fn number(el: &Element, name: &str) -> Result<Option<f64>, Error> {
    match el.find_element(name) {
        Some(_) => Ok(Some(el.find_element_value_as_number(name)?)),
        None => Ok(None),
    }
}

/// `el` with the engine functions JSBSim looks up by name renamed under
/// the engine.
fn renamed(el: &Element) -> Element {
    let mut el = el.clone();
    for f in el
        .children_mut()
        .iter_mut()
        .filter(|c| c.name() == "function")
    {
        let name = f.attribute("name");
        if FUNCTIONS.contains(&name) {
            let name = format!("propulsion/engine[#]/{name}");
            f.set_attribute("name", name);
        }
    }
    el
}

/// A `<tsfc>` or `<atsfc>` value: a number, as `FGRealValue` or
/// `FGSimplifiedTSFC` hold it; JSBSim makes anything else an
/// `FGFunction` evaluated in the middle of `Calculate`, which the port
/// refuses.
fn parameter(el: &Element, name: &str, default: f64) -> Result<f64, Error> {
    let Some(p) = el.find_element(name) else {
        return Ok(default);
    };
    match atof_locale_c(p.data_line(0)) {
        Ok(v) => Ok(v),
        Err(_) => Err(Error::Unsupported {
            construct: format!("<{name}> function (FGTurbine)"),
            path: p.location().to_string(),
        }),
    }
}

/// The constructor's defaults under what the engine file sets.
fn read_config(el: &Element) -> Result<TurbineConfig, Error> {
    let get = |name: &str, default: f64| Ok::<f64, Error>(number(el, name)?.unwrap_or(default));
    let int = |name: &str| Ok::<i32, Error>(number(el, name)?.map_or(0, |v| v as i32));
    let mil_thrust = match el.find_element("milthrust") {
        Some(_) => el.find_element_value_as_number_convert_to("milthrust", "LBS")?,
        None => 10000.0,
    };
    let max_thrust = match el.find_element("maxthrust") {
        Some(_) => el.find_element_value_as_number_convert_to("maxthrust", "LBS")?,
        None => 10000.0,
    };
    let disable_windmill = match el.find_element("disable-windmill") {
        Some(_) => el.find_element_value_as_boolean("disable-windmill")?,
        None => false,
    };
    Ok(TurbineConfig {
        mil_thrust,
        max_thrust,
        bypass_ratio: get("bypassratio", 0.0)?,
        tsfc: parameter(el, "tsfc", 0.8)?,
        atsfc: parameter(el, "atsfc", 1.7)?,
        idle_n1: get("idlen1", 30.0)?,
        idle_n2: get("idlen2", 60.0)?,
        ignition_n1: get("ignitionn1", 5.21)?,
        ignition_n2: get("ignitionn2", 25.18)?,
        n1_spinup: get("n1spinup", 1.0)?,
        n2_spinup: get("n2spinup", 3.0)?,
        n1_start_rate: get("n1startrate", 1.4)?,
        n2_start_rate: get("n2startrate", 2.0)?,
        n1_spindown: get("n1spindown", 2.0)?,
        n2_spindown: get("n2spindown", 2.0)?,
        augmented: int("augmented")?,
        aug_method: int("augmethod")?,
        injected: int("injected")?,
        injection_time: get("injection-time", 30.0)?,
        disable_windmill,
        idle_ff: crate::elementary::pow(mil_thrust, 0.2) * 107.0,
    })
}

/// `GetPreFunction(propulsion/engine[n]/<name>)`.
fn pre_function(cx: &mut EngineLoad<'_>, name: &str) -> Result<Option<FnId>, Error> {
    let full = format!("propulsion/engine[{}]/{name}", cx.number());
    let Some(base) = cx.base_mut() else {
        return Ok(None);
    };
    let pre = base.functions.pre.clone();
    let ctx = cx.context();
    for id in pre {
        if ctx.functions.function(id)?.name() == full {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

/// A spool rate: its function, or the default with `factor`.
fn spool(cx: &mut EngineLoad<'_>, name: &str, bpr: f64, factor: f64) -> Result<Spool, Error> {
    Ok(match pre_function(cx, name)? {
        Some(id) => Spool::Function(id),
        None => Spool::Default {
            delay: factor * 90.0 / (bpr + 3.0),
        },
    })
}

/// A lookup function `Calculate` dereferences unconditionally.
fn required(cx: &mut EngineLoad<'_>, el: &Element, name: &str) -> Result<FnId, Error> {
    pre_function(cx, name)?.ok_or_else(|| Error::Model {
        message: format!(
            "{}: turbine engine has no function {name}, which JSBSim dereferences",
            el.location()
        ),
    })
}

/// The lookups, with JSBSim's default spool rates for those the engine
/// does not define.
fn lookups(
    cx: &mut EngineLoad<'_>,
    el: &Element,
    c: &TurbineConfig,
) -> Result<TurbineLookups, Error> {
    let aug_thrust = pre_function(cx, "AugThrust")?;
    if aug_thrust.is_none() && (c.augmented == 1 || c.aug_method == 2) {
        required(cx, el, "AugThrust")?;
    }
    // An engine with injection but no `Injection` loads; `Calculate`
    // fails once injection is commanded (see `Turbine::sample`).
    let injection = pre_function(cx, "Injection")?;
    let bpr = c.bypass_ratio;
    Ok(TurbineLookups {
        idle_thrust: required(cx, el, "IdleThrust")?,
        mil_thrust: required(cx, el, "MilThrust")?,
        aug_thrust,
        injection,
        n1_spool_up: spool(cx, "N1SpoolUp", bpr, 1.0)?,
        n1_spool_down: spool(cx, "N1SpoolDown", bpr, 2.4)?,
        n2_spool_up: spool(cx, "N2SpoolUp", bpr, 1.0)?,
        n2_spool_down: spool(cx, "N2SpoolDown", bpr, 3.0)?,
    })
}

/// `FGTurbine`'s constructor on `<turbine_engine>` `el`.
pub fn load(el: &Element, cx: &mut EngineLoad<'_>) -> Result<Turbine, Error> {
    let el = &renamed(el);
    cx.load_base(el, EngineType::Turbine)?;
    let config = read_config(el)?;
    let lookups = lookups(cx, el, &config)?;
    let mut turbine = Turbine {
        number: cx.number(),
        config: Box::new(TurbineConfig {
            // `ResetToIC` (twice) runs before `Load` reads <tsfc>.
            tsfc: 0.8,
            ..config.clone()
        }),
        lookups,
        flags: TurbineFlags::default(),
        state: TurbineState {
            phase: Phase::Off,
            n1: 0.0,
            n2: 0.0,
            n2norm: 0.0,
            max_n1: 100.0,
            max_n2: 100.0,
            n1_factor: 0.0,
            n2_factor: 0.0,
            throttle_pos: 0.0,
            augment_cmd: 0.0,
            egt_degc: 0.0,
            epr: 1.0,
            oil_pressure_psi: 0.0,
            oil_temp_degk: 0.0,
            bleed_demand: 0.0,
            inlet_position: 1.0,
            nozzle_position: 1.0,
            corrected_tsfc: 0.0,
            injection_timer: 0.0,
            inj_water_norm: 0.0,
            inj_n1_increment: 0.0,
            inj_n2_increment: 0.0,
        },
    };
    if let Some((_, _, input)) = cx.loaded_mut() {
        turbine.reset_state(input);
    }
    turbine.config.tsfc = config.tsfc;
    let s = &mut turbine.state;
    if let Some(v) = number(el, "bleed")? {
        s.bleed_demand = v;
    }
    for (name, slot) in [("maxn1", &mut s.max_n1), ("maxn2", &mut s.max_n2)] {
        if let Some(v) = number(el, name)? {
            *slot = v;
        }
    }
    if el.find_element("injection-time").is_some() {
        s.inj_water_norm = 1.0;
    }
    if let Some(v) = number(el, "injection-N1-inc")? {
        s.inj_n1_increment = v;
    }
    if let Some(v) = number(el, "injection-N2-inc")? {
        s.inj_n2_increment = v;
    }
    s.n1_factor = s.max_n1 - config.idle_n1;
    s.n2_factor = s.max_n2 - config.idle_n2;
    if let Some((_, _, input)) = cx.loaded_mut() {
        turbine.state.oil_temp_degk = input.tat_c + 273.0;
    }
    let n = cx.number();
    super::bind::bind(&mut cx.context().binder, n)?;
    Ok(turbine)
}
