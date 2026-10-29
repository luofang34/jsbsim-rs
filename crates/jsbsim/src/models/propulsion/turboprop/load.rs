// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2004 Jiri "Javky" Javurek (javky@email.cz)
// Derived from JSBSim src/models/propulsion/FGTurboProp.cpp (the
// constructor, SetDefaults and Load).
//
//! Loading a `<turboprop_engine>`.

use super::{Phase, PowerVc, TurboProp, TurboPropConfig, TurboPropState};
use crate::Error;
use crate::fdm::EngineLoad;
use crate::functions::{FnId, Table, TableId};
use crate::models::propulsion::engine::EngineType;
use crate::units::constrain;
use crate::xml::Element;

/// The optional number `name` of `el`.
fn number(el: &Element, name: &str) -> Result<Option<f64>, Error> {
    match el.find_element(name) {
        Some(_) => Ok(Some(el.find_element_value_as_number(name)?)),
        None => Ok(None),
    }
}

/// `el` with its `EnginePowerVC` function renamed under the engine.
fn renamed(el: &Element) -> Element {
    let mut el = el.clone();
    for f in el
        .children_mut()
        .iter_mut()
        .filter(|c| c.name() == "function")
    {
        if f.attribute("name") == "EnginePowerVC" {
            f.set_attribute("name", "propulsion/engine[#]/EnginePowerVC");
        }
    }
    el
}

/// JSBSim's combustion efficiency by N1 when the engine has none, from
/// the approximation `9.333 - N1 / 12`.
fn default_combustion_efficiency() -> Table {
    Table::one_d(&[
        (60.0, 12.0 / 52.0),
        (82.0, 12.0 / 30.0),
        (96.0, 12.0 / 16.0),
        (100.0, 1.0),
        (104.0, 1.5),
        (110.0, 6.0),
    ])
}

/// The tables of `Load`'s `FindElement("table")` loop.
#[derive(Default)]
struct Tables {
    power_vc: Option<TableId>,
    power_rpm_n1: Option<Table>,
    itt_n1: Option<Table>,
    combustion_efficiency_n1: Option<Table>,
}

/// The tables: `EnginePowerVC` (when no function gives it) bound under
/// the engine, the others loaded as `FGTable(pm, el)`.
fn load_tables(el: &Element, cx: &mut EngineLoad<'_>, have_vc: bool) -> Result<Tables, Error> {
    let prefix = cx.number().to_string();
    let mut t = Tables::default();
    for table_el in el.find_elements("table") {
        let name = table_el.attribute("name");
        let ctx = cx.context();
        if !have_vc && t.power_vc.is_none() && name == "EnginePowerVC" {
            // Named per engine so that each binds its own property.
            let mut renamed = table_el.clone();
            renamed.set_attribute("name", "propulsion/engine[#]/EnginePowerVC");
            let id = ctx
                .functions
                .load_table(ctx.binder.tree, &renamed, &prefix)?;
            t.power_vc = Some(id);
            tracing::warn!(
                "{}: Note: Using the EnginePowerVC without enclosed <function> tag is deprecated",
                table_el.location()
            );
            continue;
        }
        let slot = match name {
            "EnginePowerRPM_N1" => &mut t.power_rpm_n1,
            "ITT_N1" => &mut t.itt_n1,
            "CombustionEfficiency_N1" => &mut t.combustion_efficiency_n1,
            _ => {
                tracing::error!(
                    "{}: Unknown table type: {name} in turboprop definition.",
                    el.location()
                );
                continue;
            }
        };
        let id = ctx.functions.load_table(ctx.binder.tree, table_el, "")?;
        *slot = Some(ctx.functions.table(id)?.clone());
    }
    Ok(t)
}

/// `GetPreFunction(propulsion/engine[n]/EnginePowerVC)`.
fn power_vc_function(cx: &mut EngineLoad<'_>) -> Result<Option<FnId>, Error> {
    let full = format!("propulsion/engine[{}]/EnginePowerVC", cx.number());
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

/// A table `Calculate` dereferences unconditionally.
fn required<T>(el: &Element, what: &str, value: Option<T>) -> Result<T, Error> {
    value.ok_or_else(|| Error::Model {
        message: format!(
            "{}: turboprop engine has no {what}, which JSBSim dereferences",
            el.location()
        ),
    })
}

/// `SetDefaults` under what the engine file sets.
fn read_config(el: &Element, tables: Tables, vc: PowerVc) -> Result<TurboPropConfig, Error> {
    let get = |name: &str, default: f64| Ok::<f64, Error>(number(el, name)?.unwrap_or(default));
    if el.find_element("idlefuelflow").is_some() {
        tracing::warn!(
            "{}: Note: 'idlefuelflow' is obsolete, use the 'CombustionEfficiency_N1' table instead.",
            el.location()
        );
    }
    let idle_n1 = get("idlen1", 30.0)?;
    let max_n1 = get("maxn1", 100.0)?;
    let beta = match number(el, "betarangeend")? {
        Some(v) => v / 100.0,
        None => 0.0,
    };
    Ok(TurboPropConfig {
        idle_n1,
        max_n1,
        n1_factor: max_n1 - idle_n1,
        beta_range_throttle_end: constrain(0.0, beta, 0.99999),
        reverse_max_power: number(el, "reversemaxpower")?.map_or(0.0, |v| v / 100.0),
        // Left uninitialized by JSBSim when the file does not set them.
        max_power: get("maxpower", 0.0)?,
        psfc: get("psfc", 0.0)?,
        idle_max_delay: get("n1idle_max_delay", 1.0)?,
        max_starting_time: get("maxstartingtime", 999999.0)?,
        starter_n1: get("startern1", 0.0)?,
        ielu_max_torque: get("ielumaxtorque", -1.0)?,
        itt_delay: get("itt_delay", 0.05)?,
        engine_power_rpm_n1: required(el, "EnginePowerRPM_N1 table", tables.power_rpm_n1)?,
        itt_n1: required(el, "ITT_N1 table", tables.itt_n1)?,
        combustion_efficiency_n1: tables
            .combustion_efficiency_n1
            .unwrap_or_else(default_combustion_efficiency),
        engine_power_vc: vc,
    })
}

/// `FGTurboProp`'s constructor on `<turboprop_engine>` `el`.
pub fn load(el: &Element, cx: &mut EngineLoad<'_>) -> Result<TurboProp, Error> {
    let el = &renamed(el);
    cx.load_base(el, EngineType::Turboprop)?;
    let function = power_vc_function(cx)?;
    let tables = load_tables(el, cx, function.is_some())?;
    let vc = match (function, tables.power_vc) {
        (Some(id), _) => PowerVc::Function(id),
        (None, Some(id)) => PowerVc::Table(id),
        (None, None) => required(el, "EnginePowerVC", None)?,
    };
    let config = read_config(el, tables, vc)?;
    let tat_c = cx.inputs().tat_c;
    let n = cx.number();
    super::bind::bind(&mut cx.context().binder, n)?;
    Ok(TurboProp {
        number: n,
        config: Box::new(config),
        state: TurboPropState {
            phase: Phase::Off,
            n1: 0.0,
            hp: 0.0,
            throttle_pos: 0.0,
            old_throttle: 0.0,
            rpm: 0.0,
            oil_pressure_psi: 0.0,
            oil_temp_degk: tat_c + 273.0,
            combustion_efficiency: 1.0,
            start_time: 0.0,
            eng_itt_degc: 0.0,
            eng_temperature: 0.0,
            reversed: false,
            cutoff: true,
            ielu_intervent: false,
            eng_starting: false,
            generator_power: true,
            condition: 0,
        },
    })
}
