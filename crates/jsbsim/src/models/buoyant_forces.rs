// SPDX-License-Identifier: LGPL-2.1-or-later
// FGBuoyantForces: Copyright (C) 2006 - 2013 Anders Gidenstam.
// Derived from JSBSim src/models/FGBuoyantForces.{h,cpp}.
//
//! Buoyancy of lighter-than-air vehicles: the gas cells' lift, and their
//! gas's mass, moment and inertia for `FGMassBalance`.

pub mod ballonet;
mod bind;
pub mod gas_cell;
pub mod shape;
#[cfg(test)]
mod tests;

pub use gas_cell::GasCell;

use super::base::ModelBase;
use crate::Error;
use crate::fdm::LoadContext;
use crate::math::{ColumnVector3, Matrix33};
use crate::xml::Element;

/// `FGGasCell::R` (and `FGBallonet::R`), lbs·ft/(mol·°R).
pub const GAS_R: f64 = 3.4071;
/// `FGGasCell::M_air` (and `FGBallonet::M_air`), slug/mol.
pub const M_AIR: f64 = 0.0019186;

/// `FGGasCell::Inputs`, which the executive loads from the atmosphere and
/// the gravity.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inputs {
    /// Ambient pressure, psf.
    pub pressure: f64,
    /// Ambient temperature, °R.
    pub temperature: f64,
    /// Ambient density, slug/ft^3.
    pub density: f64,
    /// Magnitude of the gravity acceleration, ft/s^2.
    pub gravity: f64,
}

/// `FGBuoyantForces`.
#[derive(Debug, Clone)]
pub struct BuoyantForces {
    /// Scheduling.
    pub base: ModelBase,
    /// Inputs of the current frame, shared by every cell.
    pub input: Inputs,
    /// The gas cells.
    pub cells: Vec<GasCell>,
    /// `vTotalForces`, body frame, lbs.
    pub forces: ColumnVector3,
    /// `vTotalMoments`, lbs·ft.
    pub moments: ColumnVector3,
    /// `gasCellJ`: the inertia last summed (zero without cells).
    gas_cell_j: Matrix33,
    /// `NoneDefined`.
    pub none_defined: bool,
}

impl Default for BuoyantForces {
    fn default() -> Self {
        Self {
            base: ModelBase::default(),
            input: Inputs::default(),
            cells: Vec::new(),
            forces: ColumnVector3::zero(),
            moments: ColumnVector3::zero(),
            gas_cell_j: Matrix33::zero(),
            none_defined: true,
        }
    }
}

impl BuoyantForces {
    /// `InitModel`: the totals are cleared; the cells keep their state.
    pub fn init_model(&mut self) {
        self.base.init();
        self.forces = ColumnVector3::zero();
        self.moments = ColumnVector3::zero();
    }

    /// The `<gas_cell>` elements of `Load`, each tied and given its
    /// functions as it is constructed, ballonets after their cell.
    pub fn load(&mut self, el: &Element, ctx: &mut LoadContext<'_>) -> Result<(), Error> {
        for cell_el in el.find_elements("gas_cell") {
            self.none_defined = false;
            let index = self.cells.len();
            let mut cell = GasCell::load(cell_el, &self.input)?;
            bind::bind_cell(&mut ctx.binder, index)?;
            cell.load_heat(cell_el, ctx)?;
            self.cells.push(cell);
            for b_el in cell_el.find_elements("ballonet") {
                let Some(cell) = self.cells.get_mut(index) else {
                    break;
                };
                let k = cell.ballonets.len();
                let ballonet = ballonet::Ballonet::load(b_el, cell.temperature, cell.pressure)?;
                cell.ballonets.push(ballonet);
                bind::bind_ballonet(&mut ctx.binder, index, k)?;
                if let Some(b) = self
                    .cells
                    .get_mut(index)
                    .and_then(|c| c.ballonets.get_mut(k))
                {
                    b.load_functions(b_el, ctx)?;
                }
            }
        }
        Ok(())
    }

    /// `bind`, which `Load` calls once the cells are loaded, if any are.
    pub fn bind(&self, b: &mut crate::fdm::Binder<'_>) -> Result<(), Error> {
        if self.none_defined {
            return Ok(());
        }
        bind::bind_totals(b)
    }

    /// `GetGasMass`, slugs.
    #[must_use]
    pub fn gas_mass(&self) -> f64 {
        let mut gw = 0.0;
        for c in &self.cells {
            gw += c.mass;
        }
        gw
    }

    /// `GetGasMassMoment`, structural frame, lbs·in.
    #[must_use]
    pub fn gas_mass_moment(&self) -> ColumnVector3 {
        let mut arm = ColumnVector3::zero();
        for c in &self.cells {
            arm += c.mass_moment;
        }
        arm
    }

    /// `GetGasMassInertia`, body frame, slug·ft^2.
    pub fn gas_mass_inertia(&mut self) -> Matrix33 {
        if self.cells.is_empty() {
            return self.gas_cell_j;
        }
        let mut j = Matrix33::zero();
        for c in &self.cells {
            j += c.inertia;
        }
        self.gas_cell_j = j;
        j
    }
}
