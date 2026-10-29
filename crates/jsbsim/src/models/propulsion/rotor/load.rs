// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2010 T. Kreitler (t.kreitler@web.de)
// Derived from JSBSim src/models/propulsion/FGRotor.cpp (the constructor,
// ConfigValueConv and Configure).
//
//! Loading a `<rotor>`: the parameters, with estimates for those it
//! leaves out, the speed's source and the transmission.

use std::f64::consts::PI;

use super::{
    ControlMap, ExternalRpm, Rotor, RotorConfig, RotorControls, RotorState, RpmDrive, bind,
};
use crate::Error;
use crate::fdm::LoadContext;
use crate::math::ColumnVector3;
use crate::models::propulsion::families::ThrusterSite;
use crate::models::propulsion::lag_filter::LagFilter;
use crate::models::propulsion::thruster::ThrusterBase;
use crate::models::propulsion::transmission::{self, Transmission};
use crate::units::{HPTOFTLBSSEC, constrain};
use crate::xml::Element;

fn sqr(x: f64) -> f64 {
    x * x
}

/// `ConfigValueConv`: the child `name` of `el` in `unit` (as written when
/// `unit` is empty), or `default`; with `tell`, a missing child is logged.
fn value_conv(
    el: &Element,
    name: &str,
    default: f64,
    unit: &str,
    tell: bool,
) -> Result<f64, Error> {
    match el.find_element(name) {
        Some(e) if unit.is_empty() => Ok(e.data_as_number()?),
        Some(_) => Ok(el.find_element_value_as_number_convert_to(name, unit)?),
        None => {
            if tell {
                tracing::error!(
                    "{}: {}: missing element '{name}' using estimated value: {default}",
                    el.location(),
                    el.name()
                );
            }
            Ok(default)
        }
    }
}

/// `ConfigValue`: [`value_conv`] without a unit.
fn value(el: &Element, name: &str, default: f64, tell: bool) -> Result<f64, Error> {
    value_conv(el, name, default, "", tell)
}

/// `Configure`'s reads, estimates and derived parameters, given the
/// load-time step `dt`; and its estimate of the engine power.
fn configure(el: &Element, base: &mut ThrusterBase, dt: f64) -> Result<(RotorConfig, f64), Error> {
    let (yell, silent) = (true, false);
    let radius = constrain(
        1e-3,
        0.5 * value_conv(el, "diameter", 42.0, "FT", yell)?,
        1e9,
    );
    let blade_num = value(el, "numblades", 3.0, yell)? as i32;
    base.gear_ratio = constrain(1e-9, value(el, "gearratio", 1.0, yell)?, 1e9);
    // A tip speed below 0.7 Mach, about 750 ft/s.
    let estimate = (750.0 / radius) / (2.0 * PI) * 60.0;
    let nominal_rpm = constrain(2.0, value(el, "nominalrpm", estimate, yell)?, 1e9);
    let minimal_rpm = constrain(1.0, value(el, "minrpm", 1.0, false)?, nominal_rpm - 1.0);
    let maximal_rpm = constrain(
        nominal_rpm,
        value(el, "maxrpm", 2.0 * nominal_rpm, false)?,
        1e9,
    );
    // A guessed solidity.
    let estimate = constrain(0.07, 2.0 / radius, 0.14);
    let estimate = estimate * PI * radius / f64::from(blade_num);
    let blade_chord = value_conv(el, "chord", estimate, "FT", yell)?;
    let lift_curve_slope = value(el, "liftcurveslope", 6.0, false)?;
    let blade_twist = value_conv(el, "twist", -0.17, "RAD", false)?;
    let hinge_offset = value_conv(el, "hingeoffset", 0.05 * radius, "FT", false)?;
    let estimate = sqr(blade_chord) * sqr(radius - hinge_offset) * 0.57;
    let flapping = value_conv(el, "flappingmoment", estimate, "SLUG*FT2", false)?;
    let blade_flapping_moment = constrain(1e-9, flapping, 1e9);
    // The mass of a thin stick of that moment, times the blade's CG
    // distance.
    let estimate = (3.0 * blade_flapping_moment / sqr(radius)) * (0.45 * radius);
    let blade_mass_moment = constrain(1e-9, value(el, "massmoment", estimate, false)?, 1e9);
    let estimate = 1.1 * blade_flapping_moment * f64::from(blade_num);
    let polar = value_conv(el, "polarmoment", estimate, "SLUG*FT2", false)?;
    let polar_moment = constrain(1e-9, polar, 1e9);
    let tip_loss_b = value(el, "tiplossfactor", 1.0, silent)?;
    let engine_power_est = 0.5 * f64::from(blade_num) * blade_chord * radius * radius;
    let estimate = engine_power_est / 30.0;
    let max_brake_power = value_conv(el, "maxbrakepower", estimate, "HP", false)? * HPTOFTLBSSEC;
    let ground_effect_exp = value(el, "groundeffectexp", 0.0, false)?;
    let ground_effect_shift = value_conv(el, "groundeffectshift", 0.0, "FT", false)?;
    let mut config = RotorConfig {
        dt,
        radius,
        blade_num,
        sense: 1.0,
        nominal_rpm,
        minimal_rpm,
        maximal_rpm,
        blade_chord,
        lift_curve_slope,
        blade_twist,
        hinge_offset,
        blade_flapping_moment,
        blade_mass_moment,
        polar_moment,
        inflow_lag: 0.0,
        tip_loss_b,
        ground_effect_exp,
        ground_effect_shift,
        lock_number_by_rho: 0.0,
        solidity: 0.0,
        r: [0.0; 5],
        b: [0.0; 5],
        max_brake_power,
        gear_loss: 0.0,
        gear_moment: 0.0,
    };
    derive(&mut config, el)?;
    Ok((config, engine_power_est))
}

/// The rest of `Configure`: the powers of the radius and tip loss, the
/// Lock number, the solidity and the inflow lag, estimated from them.
fn derive(c: &mut RotorConfig, el: &Element) -> Result<(), Error> {
    let r1 = c.radius;
    let r2 = r1 * r1;
    let r3 = r2 * r1;
    c.r = [1.0, r1, r2, r3, r3 * r1];
    let b1 = c.tip_loss_b;
    let b2 = b1 * b1;
    let b3 = b2 * b1;
    c.b = [1.0, b1, b2, b3, b3 * b1];
    c.lock_number_by_rho = c.lift_curve_slope * c.blade_chord * c.r[4] / c.blade_flapping_moment;
    c.solidity = f64::from(c.blade_num) * c.blade_chord / (PI * c.radius);
    // /GE49/ eqn(1): 16 / (gamma Omega) at sea level.
    let omega_tmp = (c.nominal_rpm / 60.0) * 2.0 * PI;
    let estimate = 16.0 / (c.lock_number_by_rho * 0.002356 * omega_tmp);
    c.inflow_lag = constrain(1e-6, value(el, "inflowlag", estimate, true)?, 2.0);
    Ok(())
}

/// The `<sense>` of the thruster: CW (-1), coaxial (0) or CCW (1) seen
/// from above.
fn sense(thruster_el: &Element) -> Result<f64, Error> {
    let Some(el) = thruster_el.find_element("sense") else {
        return Ok(1.0);
    };
    let s = el.data_as_number()?;
    Ok(if s < -0.1 {
        -1.0
    } else if s < 0.1 {
        0.0
    } else {
        1.0
    })
}

/// The control map of `<controlmap>`, main for anything unknown.
fn control_map(el: &Element) -> ControlMap {
    let Some(map_el) = el.find_element("controlmap") else {
        return ControlMap::Main;
    };
    match el.find_element_value("controlmap").to_uppercase().as_str() {
        "TAIL" => ControlMap::Tail,
        "TANDEM" => ControlMap::Tandem,
        cm => {
            tracing::error!(
                "{}: # found unknown controlmap: '{cm}' using main rotor config.",
                map_el.location()
            );
            ControlMap::Main
        }
    }
}

/// `<ExternalRPM>`: the engine whose rotor dictates the speed, or -1 for
/// a property; the rotor's own engine and engines not loaded yet are
/// replaced by -1.
fn external_rpm(
    el: &Element,
    site: &ThrusterSite<'_>,
    num: usize,
) -> Result<Option<ExternalRpm>, Error> {
    let Some(ext_el) = el.find_element("ExternalRPM") else {
        return Ok(None);
    };
    let given = el.find_element_value_as_number("ExternalRPM")? as i32;
    let mut ext = ExternalRpm {
        definition: given,
        source: None,
        source_gear_ratio: 1.0,
    };
    if let Ok(source) = usize::try_from(given) {
        match site.engines.get(source) {
            Some(engine) if source != num => {
                ext.source_gear_ratio = engine.thruster.base.gear_ratio;
            }
            _ => ext.definition = -1,
        }
    }
    if ext.definition != given {
        tracing::error!(
            "{}: # discarded given RPM source ({given}) and switched to external control (-1).",
            ext_el.location()
        );
    }
    Ok(Some(ext))
}

impl Rotor {
    /// `FGRotor(exec, rotor_element, num)` after `FGThruster`'s
    /// constructor: the sense, location and orientation of the
    /// `<thruster>`, the controls, the speed's source, `Configure`, the
    /// transmission of a driven rotor, then the ties.
    pub(super) fn load_rotor(
        el: &Element,
        thruster_el: &Element,
        base: &mut ThrusterBase,
        ctx: &mut LoadContext<'_>,
        site: &ThrusterSite<'_>,
    ) -> Result<Self, Error> {
        let num = base.engine_number;
        let location = match thruster_el.find_element("location") {
            Some(loc) => loc.find_element_triplet_convert_to("IN")?,
            None => {
                tracing::error!("{}: No thruster location found.", el.location());
                ColumnVector3::zero()
            }
        };
        let orientation = match thruster_el.find_element("orient") {
            Some(orient) => orient.find_element_triplet_convert_to("RAD")?,
            None => {
                tracing::error!("{}: No thruster orientation found.", el.location());
                ColumnVector3::zero()
            }
        };
        base.force.set_location(location);
        base.force.set_angles_to_body(orientation);
        let inv_transform = base.force.transform.transposed();
        let map = control_map(el);
        let external = external_rpm(el, site, num)?;
        let (mut config, engine_power_est) = configure(el, base, site.dt)?;
        config.sense = sense(thruster_el)?;
        let drive = match external {
            Some(ext) => RpmDrive::External(ext),
            None => {
                let mut t = Transmission::new(site);
                transmission::bind(&mut ctx.binder, num)?;
                t.thruster_moment = config.polar_moment;
                let moment = value_conv(
                    el,
                    "gearmoment",
                    0.1 * config.polar_moment,
                    "SLUG*FT2",
                    false,
                )?;
                config.gear_moment = constrain(1e-6, moment, 1e9);
                t.engine_moment = config.gear_moment;
                t.max_brake_power = config.max_brake_power;
                let loss = value_conv(el, "gearloss", 0.0025 * engine_power_est, "HP", false)?;
                config.gear_loss = constrain(0.0, loss, 1e9) * HPTOFTLBSSEC;
                t.engine_friction = config.gear_loss;
                RpmDrive::Transmission(t)
            }
        };
        let mut state = RotorState::default();
        state.init_dynamics();
        let mut rotor = Self {
            damp_hagl: LagFilter::new(1.0, config.dt),
            config,
            state,
            controls: RotorControls {
                map,
                collective: 0.0,
                lateral: 0.0,
                longitudinal: 0.0,
                ground_effect_scale_norm: 1.0,
            },
            drive,
            inv_transform,
        };
        rotor.state.rho = 0.002356;
        bind::bind(&mut rotor, ctx, num)?;
        Ok(rotor)
    }
}
