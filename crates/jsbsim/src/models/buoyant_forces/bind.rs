// SPDX-License-Identifier: LGPL-2.1-or-later
// FGBuoyantForces, FGGasCell, FGBallonet: Copyright (C) 2006 - 2013 Anders
// Gidenstam.
// Derived from JSBSim src/models/FGBuoyantForces.cpp (bind) and
// src/models/FGGasCell.cpp (the constructors' ties).
//
//! The properties of the gas cells, their ballonets and the totals.

use super::GasCell;
use super::ballonet::Ballonet;
use crate::Error;
use crate::fdm::{Binder, Kind, Models};
use crate::math::{E_L, E_M, E_N, E_X, E_Y, E_Z};
use crate::models::ground_reactions::contact_bind::indexed_name;

type CellGet = fn(&GasCell) -> f64;
type CellSet = fn(&mut GasCell, f64);
type BallonetGet = fn(&Ballonet) -> f64;
type BallonetSet = fn(&mut Ballonet, f64);

/// A gas cell's ties in the constructor's order; `max_volume-ft3` has its
/// WRITE attribute cleared.
const CELL: [(&str, CellGet, Option<CellSet>); 7] = [
    ("max_volume-ft3", |c| c.envelope.max_volume, None),
    ("temp-R", |c| c.temperature, Some(|c, v| c.temperature = v)),
    ("pressure-psf", |c| c.pressure, Some(|c, v| c.pressure = v)),
    ("volume-ft3", |c| c.volume, Some(|c, v| c.volume = v)),
    ("buoyancy-lbs", |c| c.buoyancy, Some(|c, v| c.buoyancy = v)),
    ("contents-mol", |c| c.contents, Some(|c, v| c.contents = v)),
    (
        "valve_open",
        |c| c.valve_open,
        Some(|c, v| c.valve_open = v),
    ),
];

/// A ballonet's ties in the constructor's order.
const BALLONET: [(&str, BallonetGet, Option<BallonetSet>); 6] = [
    ("max_volume-ft3", |b| b.envelope.max_volume, None),
    ("temp-R", |b| b.temperature, Some(|b, v| b.temperature = v)),
    ("pressure-psf", |b| b.pressure, Some(|b, v| b.pressure = v)),
    ("volume-ft3", |b| b.volume, Some(|b, v| b.volume = v)),
    ("contents-mol", |b| b.contents, Some(|b, v| b.contents = v)),
    (
        "valve_open",
        |b| b.valve_open,
        Some(|b, v| b.valve_open = v),
    ),
];

fn cell(m: &Models, i: usize) -> Option<&GasCell> {
    m.buoyant_forces.cells.get(i)
}

fn ballonet(m: &Models, i: usize, k: usize) -> Option<&Ballonet> {
    cell(m, i)?.ballonets.get(k)
}

/// The ties of gas cell `i`.
pub fn bind_cell(b: &mut Binder<'_>, i: usize) -> Result<(), Error> {
    let base = indexed_name("buoyant_forces/gas-cell", i);
    for (suffix, get, set) in CELL {
        let path = format!("{base}/{suffix}");
        let getter = move |m: &Models| cell(m, i).map_or(0.0, get);
        match set {
            Some(set) => b.property(&path, Kind::Double, getter, move |m, v| {
                if let Some(c) = m.buoyant_forces.cells.get_mut(i) {
                    set(c, v);
                }
            })?,
            None => b.getter(&path, getter)?,
        };
    }
    Ok(())
}

/// The ties of ballonet `k` of gas cell `i`.
pub fn bind_ballonet(b: &mut Binder<'_>, i: usize, k: usize) -> Result<(), Error> {
    let base = indexed_name("buoyant_forces/gas-cell", i);
    let base = indexed_name(&format!("{base}/ballonet"), k);
    for (suffix, get, set) in BALLONET {
        let path = format!("{base}/{suffix}");
        let getter = move |m: &Models| ballonet(m, i, k).map_or(0.0, get);
        match set {
            Some(set) => b.property(&path, Kind::Double, getter, move |m, v| {
                let cells = &mut m.buoyant_forces.cells;
                if let Some(b) = cells.get_mut(i).and_then(|c| c.ballonets.get_mut(k)) {
                    set(b, v);
                }
            })?,
            None => b.getter(&path, getter)?,
        };
    }
    Ok(())
}

/// `FGBuoyantForces::bind`.
pub fn bind_totals(b: &mut Binder<'_>) -> Result<(), Error> {
    for (name, axis) in [("l", E_L), ("m", E_M), ("n", E_N)] {
        b.getter(&format!("moments/{name}-buoyancy-lbsft"), move |m| {
            m.buoyant_forces.moments[axis]
        })?;
    }
    for (name, axis) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("forces/fb{name}-buoyancy-lbs"), move |m| {
            m.buoyant_forces.forces[axis]
        })?;
    }
    Ok(())
}
