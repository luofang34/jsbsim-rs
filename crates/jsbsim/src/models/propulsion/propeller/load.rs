// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGPropeller.cpp (the
// constructor).
//
//! Loading a `<propeller>`.

use super::{Propeller, PropellerTables};
use crate::Error;
use crate::fdm::LoadContext;
use crate::functions::Table;
use crate::math::ColumnVector3;
use crate::models::propulsion::thruster::ThrusterBase;
use crate::xml::Element;

/// `std::max(value, floor)`, which keeps `value` unless it is below.
fn at_least(value: f64, floor: f64) -> f64 {
    if value < floor { floor } else { value }
}

fn model_error(el: &Element, message: &str) -> Error {
    Error::Model {
        message: format!("{}: {message}", el.location()),
    }
}

/// `FGTable(PropertyManager, el)`: loaded (and, when named and not
/// internal, bound) through the function set, of which the propeller
/// keeps its own copy to look up without the set.
fn load_table(ctx: &mut LoadContext<'_>, el: &Element) -> Result<Table, Error> {
    let id = ctx.functions.load_table(ctx.binder.tree, el, "")?;
    Ok(ctx.functions.table(id)?.clone())
}

/// The named tables of the `FindNextElement("table")` loop, which starts
/// after `<reversepitch>`, the last child the constructor looked for.
fn load_tables(el: &Element, ctx: &mut LoadContext<'_>) -> Result<Box<PropellerTables>, Error> {
    let (mut c_thrust, mut c_power) = (None, None);
    let (mut ct_mach, mut cp_mach, mut ct_rpm, mut cp_rpm) = (None, None, None, None);
    for table_el in el.find_next_elements_after("reversepitch", "table") {
        let slot = match table_el.attribute("name") {
            "C_THRUST" => &mut c_thrust,
            "C_POWER" => &mut c_power,
            "CT_MACH" => &mut ct_mach,
            "CP_MACH" => &mut cp_mach,
            "CT_RPM_FACTOR" => &mut ct_rpm,
            "CP_RPM_FACTOR" => &mut cp_rpm,
            name => {
                tracing::error!(
                    "{}: Unknown table type: {name} in propeller definition.",
                    table_el.location()
                );
                continue;
            }
        };
        *slot = Some(load_table(ctx, table_el)?);
    }
    let (Some(c_thrust), Some(c_power)) = (c_thrust, c_power) else {
        return Err(model_error(
            el,
            "Propeller configuration must contain C_THRUST and C_POWER tables!",
        ));
    };
    Ok(Box::new(PropellerTables {
        c_thrust,
        c_power,
        ct_mach,
        cp_mach,
        ct_rpm,
        cp_rpm,
    }))
}

/// The optional number `name` of `el`.
fn number(el: &Element, name: &str) -> Result<Option<f64>, Error> {
    match el.find_element(name) {
        Some(_) => Ok(Some(el.find_element_value_as_number(name)?)),
        None => Ok(None),
    }
}

/// The blade angle and speed ranges, the governor mode and the reverse
/// blade angle.
struct Governor {
    min_pitch: f64,
    max_pitch: f64,
    min_rpm: f64,
    max_rpm: f64,
    constant_speed: i32,
    reverse_pitch: f64,
}

/// The `<minpitch>` to `<reversepitch>` reads; a `<maxrpm>` makes the
/// propeller constant-speed unless `<constspeed>` says otherwise.
fn governor(el: &Element) -> Result<Governor, Error> {
    let min_pitch = number(el, "minpitch")?.unwrap_or(0.0);
    let max_pitch = number(el, "maxpitch")?.unwrap_or(0.0);
    let min_rpm = number(el, "minrpm")?.unwrap_or(0.0);
    let mut constant_speed = 0;
    let max_rpm = match number(el, "maxrpm")? {
        Some(v) => {
            constant_speed = 1;
            v
        }
        None => 0.0,
    };
    if let Some(v) = number(el, "constspeed")? {
        constant_speed = v as i32;
    }
    Ok(Governor {
        min_pitch,
        max_pitch,
        min_rpm,
        max_rpm,
        constant_speed,
        reverse_pitch: number(el, "reversepitch")?.unwrap_or(0.0),
    })
}

/// The `<sense>` (its sign) and `<p_factor>` of the `<thruster>`.
fn sense_and_p_factor(thruster_el: &Element) -> Result<(f64, f64), Error> {
    let mut sense = 1.0;
    if let Some(s) = thruster_el.find_element("sense") {
        sense = if s.data_as_number()? >= 0.0 {
            1.0
        } else {
            -1.0
        };
    }
    let mut p_factor = 0.0;
    let p_factor_el = thruster_el.find_element("p_factor");
    if let Some(p) = p_factor_el {
        p_factor = p.data_as_number()?;
    }
    if p_factor < 0.0 {
        let at = p_factor_el.unwrap_or(thruster_el);
        return Err(model_error(
            at,
            "P-Factor value in propeller configuration file must be greater than zero",
        ));
    }
    Ok((sense, p_factor))
}

/// `FGPropeller`'s constructor past `FGThruster`'s, on `<propeller>` `el`
/// inside `<thruster>` `thruster_el`.
pub fn load(
    el: &Element,
    thruster_el: &Element,
    base: &mut ThrusterBase,
    ctx: &mut LoadContext<'_>,
) -> Result<Propeller, Error> {
    base.gear_ratio = 1.0;
    // JSBSim leaves the inertia, diameter and blade count uninitialized
    // when absent; they start at 0 here.
    let mut ixx = 0.0;
    if el.find_element("ixx").is_some() {
        ixx = at_least(
            el.find_element_value_as_number_convert_to("ixx", "SLUG*FT2")?,
            1e-06,
        );
    }
    let sense_multiplier =
        if el.has_attribute("version") && el.attribute_as_number("version")? > 1.0 {
            -1.0
        } else {
            1.0
        };
    let mut diameter = 0.0;
    if el.find_element("diameter").is_some() {
        diameter = at_least(
            el.find_element_value_as_number_convert_to("diameter", "FT")?,
            0.001,
        );
    }
    let num_blades = number(el, "numblades")?.map_or(0, |v| v as i32);
    if let Some(g) = number(el, "gearratio")? {
        base.gear_ratio = at_least(g, 0.001);
    }
    let governor = governor(el)?;
    let tables = load_tables(el, ctx)?;
    let (sense, p_factor) = sense_and_p_factor(thruster_el)?;
    let ct_factor = number(el, "ct_factor")?.unwrap_or(1.0);
    let cp_factor = number(el, "cp_factor")?.unwrap_or(1.0);
    let d4 = diameter * diameter * diameter * diameter;
    let mut propeller = Propeller {
        num_blades,
        j: 0.0,
        rpm: 0.0,
        ixx,
        diameter,
        max_pitch: governor.max_pitch,
        min_pitch: governor.min_pitch,
        min_rpm: governor.min_rpm,
        max_rpm: governor.max_rpm,
        pitch: 0.0,
        p_factor,
        sense,
        sense_multiplier,
        advance: 0.0,
        excess_torque: 0.0,
        d4,
        d5: d4 * diameter,
        helical_tip_mach: 0.0,
        vinduced: 0.0,
        torque: ColumnVector3::zero(),
        tables,
        ct_factor,
        cp_factor,
        constant_speed: governor.constant_speed,
        reverse_pitch: governor.reverse_pitch,
        reversed: false,
        reverse_coef: 0.0,
        feathered: false,
    };
    propeller.init_dynamics();
    super::bind::bind(&mut ctx.binder, base.engine_number)?;
    Ok(propeller)
}
