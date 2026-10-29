// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGPiston.cpp (the
// constructor).
//
//! Loading a `<piston_engine>`.

use super::cycle::std_pressure_100k;
use super::{
    Boost, MAX_BOOST_SPEEDS, Piston, PistonConfig, PistonState, PistonTuning, STANDARD_PRESSURE,
};
use crate::Error;
use crate::fdm::EngineLoad;
use crate::functions::Table;
use crate::models::propulsion::engine::EngineType;
use crate::units::{FTTOM, IN3TOM3, INHGTOPA, PSFTOPA, constrain};
use crate::xml::Element;

/// The optional number `name` of `el`.
fn number(el: &Element, name: &str) -> Result<Option<f64>, Error> {
    match el.find_element(name) {
        Some(_) => Ok(Some(el.find_element_value_as_number(name)?)),
        None => Ok(None),
    }
}

/// The optional number `name` of `el`, converted to `unit`.
fn converted(el: &Element, name: &str, unit: &str) -> Result<Option<f64>, Error> {
    match el.find_element(name) {
        Some(_) => Ok(Some(
            el.find_element_value_as_number_convert_to(name, unit)?,
        )),
        None => Ok(None),
    }
}

/// `FGTable(n)` filled with `<<`: JSBSim's combustion efficiency by
/// equivalence ratio.
fn default_combustion_efficiency() -> Table {
    Table::one_d(&[
        (0.00, 0.980),
        (0.90, 0.980),
        (1.00, 0.970),
        (1.05, 0.950),
        (1.10, 0.900),
        (1.15, 0.850),
        (1.20, 0.790),
        (1.30, 0.700),
        (1.40, 0.630),
        (1.50, 0.570),
        (1.60, 0.525),
        (2.00, 0.345),
    ])
}

/// JSBSim's mixture efficiency by fuel/air ratio.
fn default_mixture_efficiency() -> Table {
    Table::one_d(&[
        (0.05000, 0.00000),
        (0.05137, 0.00862),
        (0.05179, 0.21552),
        (0.05430, 0.48276),
        (0.05842, 0.70690),
        (0.06312, 0.83621),
        (0.06942, 0.93103),
        (0.07786, 1.00000),
        (0.08845, 1.00000),
        (0.09270, 0.98276),
        (0.10120, 0.93103),
        (0.11455, 0.72414),
        (0.12158, 0.45690),
        (0.12435, 0.23276),
        (0.12500, 0.00000),
    ])
}

/// The `COMBUSTION` and `MIXTURE` tables of the `FindNextElement("table")`
/// loop, which starts after `<oil-viscosity-index>`, the last child the
/// constructor looked for; JSBSim's defaults for those absent.
fn load_tables(el: &Element, cx: &mut EngineLoad<'_>) -> Result<(Table, Table), Error> {
    let (mut combustion, mut mixture) = (None, None);
    for table_el in el.find_next_elements_after("oil-viscosity-index", "table") {
        let slot = match table_el.attribute("name") {
            "COMBUSTION" => &mut combustion,
            "MIXTURE" => &mut mixture,
            name => {
                tracing::error!("Unknown table type: {name} in piston engine definition.");
                continue;
            }
        };
        let ctx = cx.context();
        let id = ctx.functions.load_table(ctx.binder.tree, table_el, "")?;
        *slot = Some(ctx.functions.table(id)?.clone());
    }
    Ok((
        combustion.unwrap_or_else(default_combustion_efficiency),
        mixture.unwrap_or_else(default_mixture_efficiency),
    ))
}

/// The constructor's defaults (a Lycoming O-360, more or less) under what
/// the engine file sets, before the derived values.
fn read_config(
    el: &Element,
    cx: &mut EngineLoad<'_>,
) -> Result<(PistonConfig, PistonTuning), Error> {
    let get = |name: &str, default: f64| Ok::<f64, Error>(number(el, name)?.unwrap_or(default));
    let conv = |name: &str, unit: &str, default: f64| {
        Ok::<f64, Error>(converted(el, name, unit)?.unwrap_or(default))
    };
    let tuning = PistonTuning {
        static_friction_hp: conv("static-friction", "HP", 1.5)?,
        isfc: conv("bsfc", "LBS/HP*HR", -1.0)?,
        starter_gain: 1.0,
        volumetric_efficiency: get("volumetric-efficiency", 0.85)?,
        z_airbox: get("air-intake-impedance-factor", -999.0)?,
        ram_air_factor: get("ram-air-factor", 1.0)?,
        cooling_factor: get("cooling-factor", 0.5144444)?,
    };
    let max_rpm = get("maxrpm", 2800.0)?;
    let spark_fail_drop = match number(el, "sparkfaildrop")? {
        Some(v) => constrain(0.0, 1.0 - v, 1.0),
        None => 1.0,
    };
    let config = PistonConfig {
        cycles: get("cycles", 4.0)?,
        idle_rpm: get("idlerpm", 600.0)?,
        max_rpm,
        displacement: conv("displacement", "IN3", 360.0)?,
        displacement_si: 0.0,
        max_hp: conv("maxhp", "HP", 200.0)?,
        spark_fail_drop,
        min_manifold_pressure_inhg: conv("minmp", "INHG", 6.5)?,
        max_manifold_pressure_inhg: conv("maxmp", "INHG", 28.5)?,
        manifold_pressure_lag: get("man-press-lag", 1.0)?,
        bore: conv("bore", "IN", 5.125)?,
        stroke: conv("stroke", "IN", 4.375)?,
        cylinders: get("cylinders", 4.0)?,
        cylinder_head_mass: conv("cylinder-head-mass", "KG", 2.0)?,
        compression_ratio: get("compression-ratio", 8.5)?,
        z_throttle: 0.0,
        peak_mean_piston_speed_fps: get("peak-piston-speed", 100.0)?,
        rated_mean_piston_speed_fps: 0.0,
        starter_torque: get("starter-torque", -1.0)?,
        starter_rpm: get("starter-rpm", -1.0)?,
        fmep_dynamic: conv("dynamic-fmep", "PA", 18400.0)?,
        fmep_static: conv("static-fmep", "PA", 46500.0)?,
        min_map: 21950.0,
        max_map: 96250.0,
        oil_press_relief_valve: conv("oil-pressure-relief-valve-psi", "PSI", 60.0)?,
        oil_press_rpm_max: get("oil-pressure-rpm-max", max_rpm * 0.75)?,
        design_oil_temp: conv("design-oil-temp-degK", "DEGK", 358.0)?,
        oil_viscosity_index: get("oil-viscosity-index", 0.25)?,
        combustion_efficiency: Table::one_d(&[]),
        mixture_efficiency: Table::one_d(&[]),
    };
    let (combustion, mixture) = load_tables(el, cx)?;
    Ok((
        PistonConfig {
            combustion_efficiency: combustion,
            mixture_efficiency: mixture,
            ..config
        },
        tuning,
    ))
}

/// The supercharger settings, read when `<numboostspeeds>` is present.
fn read_boost(el: &Element) -> Result<Boost, Error> {
    let mut b = Boost {
        speeds: 0,
        speed: 0,
        boosted: false,
        manual: 0,
        b_manual: false,
        override_: 0,
        b_override: false,
        b_takeoff: false,
        takeoff_boost: 0.0,
        rated_boost: [0.0; MAX_BOOST_SPEEDS],
        rated_altitude: [0.0; MAX_BOOST_SPEEDS],
        rated_rpm: [2500.0; MAX_BOOST_SPEEDS],
        rated_power: [0.0; MAX_BOOST_SPEEDS],
        switch_altitude: [0.0; MAX_BOOST_SPEEDS - 1],
        switch_pressure: [0.0; MAX_BOOST_SPEEDS - 1],
        mul: [1.0; MAX_BOOST_SPEEDS],
        rated_map: [100000.0; MAX_BOOST_SPEEDS],
        takeoff_map: [100000.0; MAX_BOOST_SPEEDS],
        switch_hysteresis: 0.0,
        loss_factor: 0.0,
        loss_hp: 0.0,
    };
    let Some(speeds) = number(el, "numboostspeeds")? else {
        return Ok(b);
    };
    b.speeds = speeds as i32;
    if let Some(v) = number(el, "boostoverride")? {
        b.override_ = v as i32;
    }
    if let Some(v) = number(el, "boostmanual")? {
        b.manual = v as i32;
    }
    b.takeoff_boost = converted(el, "takeoffboost", "PSI")?.unwrap_or(0.0);
    b.loss_factor = number(el, "boost-loss-factor")?.unwrap_or(0.0);
    for i in 0..MAX_BOOST_SPEEDS {
        let k = i + 1;
        let set = |slot: &mut f64, v: Option<f64>| {
            if let Some(v) = v {
                *slot = v;
            }
        };
        if let Some(slot) = b.rated_boost.get_mut(i) {
            set(slot, converted(el, &format!("ratedboost{k}"), "PSI")?);
        }
        if let Some(slot) = b.rated_power.get_mut(i) {
            set(slot, converted(el, &format!("ratedpower{k}"), "HP")?);
        }
        if let Some(slot) = b.rated_rpm.get_mut(i) {
            set(slot, number(el, &format!("ratedrpm{k}"))?);
        }
        if let Some(slot) = b.rated_altitude.get_mut(i) {
            set(slot, converted(el, &format!("ratedaltitude{k}"), "FT")?);
        }
    }
    Ok(b)
}

/// The values the constructor derives from the configuration: the
/// starter's defaults, the swept volume, the rated piston speed, the BSFC
/// that matches the rated power when none is given, the manifold pressure
/// range and the intake and throttle impedances.
fn derive(c: &mut PistonConfig, t: &mut PistonTuning) {
    if c.starter_rpm < 0.0 {
        c.starter_rpm = 2.0 * c.idle_rpm;
    }
    if c.starter_torque < 0.0 {
        // A guess.
        c.starter_torque = c.max_hp * 0.4;
    }
    c.displacement_si = c.displacement * IN3TOM3;
    // 2 * (RPM/60) * (Stroke/12), the mean piston speed 2NS.
    c.rated_mean_piston_speed_fps = (c.max_rpm * c.stroke) / 360.0;
    if t.isfc < 0.0 {
        let mut pmep = 29.92 - c.max_manifold_pressure_inhg;
        pmep *= INHGTOPA * t.volumetric_efficiency;
        let fmep = c.fmep_dynamic * c.rated_mean_piston_speed_fps * FTTOM + c.fmep_static;
        let hp_loss = ((pmep + fmep) * c.displacement_si * c.max_rpm) / (c.cycles * 22371.0);
        t.isfc = (1.1
            * c.displacement
            * c.max_rpm
            * t.volumetric_efficiency
            * (c.max_manifold_pressure_inhg / 29.92))
            / (9411.0 * (c.max_hp + hp_loss - t.static_friction_hp));
    }
    // No boosting with a bogus maximum.
    if c.max_manifold_pressure_inhg > 29.9 {
        c.max_manifold_pressure_inhg = 29.9;
    }
    c.min_map = c.min_manifold_pressure_inhg * INHGTOPA;
    c.max_map = c.max_manifold_pressure_inhg * INHGTOPA;
    // Pm = Ze / (Ze + Zi + Zt) * Pa, the engine impedance Ze being 1 over
    // the mean piston speed; fully open, Zt = 0.
    if t.z_airbox < 0.0 {
        let ze = c.peak_mean_piston_speed_fps / c.rated_mean_piston_speed_fps;
        t.z_airbox = (STANDARD_PRESSURE * ze / c.max_map) - ze;
    }
    c.z_throttle = (c.peak_mean_piston_speed_fps / ((c.idle_rpm * c.stroke) / 360.0))
        * (STANDARD_PRESSURE / c.min_map - 1.0)
        - t.z_airbox;
}

/// Speed `i`'s switch to the next: at least 1000 ft above its rated
/// altitude, as a pressure, with a guessed hysteresis.
fn set_switch(b: &mut Boost, i: usize, altitude: f64) {
    if let Some(switch) = b.switch_altitude.get_mut(i) {
        if *switch < altitude {
            *switch = altitude + 1000.0;
        }
        let pressure = std_pressure_100k(*switch) * PSFTOPA;
        if let Some(p) = b.switch_pressure.get_mut(i) {
            *p = pressure;
        }
    }
    b.switch_hysteresis = 1000.0;
}

/// The supercharger's sanity checks and derived pressures: a speed with
/// no rated boost or power, or rated below the previous one, is dropped
/// with those above it.
fn finish_boost(b: &mut Boost) {
    let rated_boost0 = b.rated_boost[0];
    if b.takeoff_boost > rated_boost0 {
        b.b_takeoff = true;
    }
    let speeds = |b: &Boost| usize::try_from(b.speeds).unwrap_or(0);
    let mut i = 0;
    while i < speeds(b) && i < MAX_BOOST_SPEEDS {
        let at = |v: &[f64; MAX_BOOST_SPEEDS], i: usize| v.get(i).copied().unwrap_or(0.0);
        let (boost, power, altitude) = (
            at(&b.rated_boost, i),
            at(&b.rated_power, i),
            at(&b.rated_altitude, i),
        );
        let below_previous = i
            .checked_sub(1)
            .is_some_and(|p| altitude < at(&b.rated_altitude, p));
        // An altitude of 0 is unregulated supercharging.
        if boost <= 0.0 || power <= 0.0 || altitude < 0.0 || below_previous {
            b.speeds -= 1;
            break;
        }
        if i + 1 < speeds(b) {
            set_switch(b, i, altitude);
        }
        let rated_map = STANDARD_PRESSURE + boost * 6895.0;
        let takeoff_map = if b.takeoff_boost > rated_boost0 {
            // The same effect on the BCV at every speed.
            b.b_takeoff = true;
            rated_map + ((b.takeoff_boost - rated_boost0) * 6895.0)
        } else {
            b.b_takeoff = false;
            rated_map
        };
        let mul = rated_map / (std_pressure_100k(altitude) * PSFTOPA);
        for (slot, v) in [
            (b.rated_map.get_mut(i), rated_map),
            (b.takeoff_map.get_mut(i), takeoff_map),
            (b.mul.get_mut(i), mul),
        ] {
            if let Some(slot) = slot {
                *slot = v;
            }
        }
        i += 1;
    }
    if b.speeds > 0 {
        b.boosted = true;
        b.speed = 0;
    }
    b.b_override = b.override_ == 1;
    b.b_manual = b.manual == 1;
}

/// `FGPiston`'s constructor on `<piston_engine>` `el`.
pub fn load(el: &Element, cx: &mut EngineLoad<'_>) -> Result<Piston, Error> {
    cx.load_base(el, EngineType::Piston)?;
    let (mut config, mut tuning) = read_config(el, cx)?;
    let max_throttle = number(el, "maxthrottle")?;
    let min_throttle = number(el, "minthrottle")?;
    let mut boost = read_boost(el)?;
    derive(&mut config, &mut tuning);
    let n = cx.number();
    super::bind::bind(&mut cx.context().binder, n, boost.loss_factor > 0.0)?;
    finish_boost(&mut boost);
    let mut piston = Piston {
        number: n,
        config: Box::new(config),
        state: PistonState {
            volumetric_efficiency_reduced: tuning.volumetric_efficiency,
            ..PistonState::default()
        },
        tuning,
        boost,
    };
    if let Some((base, thruster, input)) = cx.loaded_mut() {
        if let Some(v) = max_throttle {
            base.max_throttle = v;
        }
        if let Some(v) = min_throttle {
            base.min_throttle = v;
        }
        piston.reset_state(thruster, input);
    }
    Ok(piston)
}
