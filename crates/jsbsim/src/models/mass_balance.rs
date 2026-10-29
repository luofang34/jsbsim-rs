// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGMassBalance.{h,cpp}.
//
//! Mass, centre of gravity and inertia (`FGMassBalance`): the empty
//! vehicle plus point masses, tanks and lifting gas.

mod bind;
mod load;
mod point_mass;

#[cfg(test)]
mod tests;

pub use bind::bind;
pub use point_mass::{PointMass, Shape};

use super::base::ModelBase;
use super::propagate::Propagate;
use crate::math::{ColumnVector3, E_X, E_Y, E_Z, Matrix33};
use crate::units::{INCHTOFT, LBTOSLUG, SLUGTOLB};

/// What other models give the mass balance (`FGMassBalance::Inputs`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Inputs {
    /// Lifting gas mass, slugs.
    pub gas_mass: f64,
    /// Tank contents, lbs.
    pub tanks_weight: f64,
    /// Lifting gas moment, lbs·in, structural frame.
    pub gas_moment: ColumnVector3,
    /// Lifting gas inertia, slug·ft^2, body frame.
    pub gas_inertia: Matrix33,
    /// Tank contents moment, lbs·in, structural frame.
    pub tanks_moment: ColumnVector3,
    /// Tank contents inertia, slug·ft^2, body frame.
    pub tank_inertia: Matrix33,
    /// Weight on wheels.
    pub wow: bool,
}

/// Mass properties.
#[derive(Debug, Clone, Default)]
pub struct MassBalance {
    /// Scheduling.
    pub base: ModelBase,
    /// Inputs, loaded by the executive before each run.
    pub input: Inputs,
    /// Mass, slugs.
    pub mass: f64,
    /// Weight, lbs.
    pub weight: f64,
    /// Empty weight, lbs.
    pub empty_weight: f64,
    /// CG, structural frame, in.
    pub xyz_cg: ColumnVector3,
    /// Inertia tensor, body frame, slug·ft^2.
    pub j: Matrix33,
    /// Its inverse.
    pub jinv: Matrix33,
    pm_j: Matrix33,
    base_j: Matrix33,
    last_xyz_cg: ColumnVector3,
    delta_xyz_cg: ColumnVector3,
    delta_xyz_cg_body: ColumnVector3,
    base_xyz_cg: ColumnVector3,
    point_masses: Vec<PointMass>,
}

impl MassBalance {
    /// `InitModel`.
    pub fn init_model(&mut self) {
        self.base.init();
        // No CG seen yet; zero would be a valid CG at the structural origin.
        self.last_xyz_cg = ColumnVector3::new(f64::NAN, f64::NAN, f64::NAN);
        self.delta_xyz_cg = ColumnVector3::zero();
    }

    /// `Run`. A CG shift while held down or on the ground moves the
    /// vehicle so that the structural frame stays put relative to it.
    pub fn run(&mut self, holding: bool, hold_down: bool, propagate: &mut Propagate) {
        if self.base.skip() || holding {
            return;
        }
        self.run_frame(hold_down, propagate);
    }

    /// `Run` once scheduling has let it through, between the pre- and
    /// post-functions the executive caches.
    pub fn run_frame(&mut self, hold_down: bool, propagate: &mut Propagate) {
        self.update_weight();
        self.xyz_cg = (self.empty_weight * self.base_xyz_cg
            + self.point_mass_moment()
            + self.input.tanks_moment
            + self.input.gas_moment)
            / self.weight;

        if self.last_xyz_cg[E_X].is_nan() {
            self.last_xyz_cg = self.xyz_cg;
        }
        self.delta_xyz_cg = self.xyz_cg - self.last_xyz_cg;
        self.delta_xyz_cg_body =
            self.structural_to_body(self.last_xyz_cg) - self.structural_to_body(self.xyz_cg);
        self.last_xyz_cg = self.xyz_cg;

        if hold_down || self.input.wow {
            propagate.nudge_body_location(self.delta_xyz_cg_body);
        }

        self.j = self.base_j;
        self.j += self.pointmass_inertia(LBTOSLUG * self.empty_weight, self.base_xyz_cg);
        let pm_j = self.calculate_pm_inertias();
        self.j += pm_j;
        self.j += self.input.tank_inertia;
        self.j += self.input.gas_inertia;
        self.jinv = inverse_inertia(&self.j);
    }

    /// The total weight and mass. JSBSim adds the weight of mated child
    /// FDMs, which this port does not have.
    fn update_weight(&mut self) {
        let child_fdm_weight = 0.0;
        self.weight = self.empty_weight
            + self.input.tanks_weight
            + self.total_point_mass_weight()
            + self.input.gas_mass * SLUGTOLB
            + child_fdm_weight;
        self.mass = LBTOSLUG * self.weight;
    }

    /// `GetMass`, slugs.
    #[must_use]
    pub fn mass(&self) -> f64 {
        self.mass
    }

    /// `GetWeight`, lbs.
    #[must_use]
    pub fn weight(&self) -> f64 {
        self.weight
    }

    /// `GetEmptyWeight`, lbs.
    #[must_use]
    pub fn empty_weight(&self) -> f64 {
        self.empty_weight
    }

    /// `GetXYZcg`: the CG in the structural frame, in.
    #[must_use]
    pub fn xyz_cg(&self) -> ColumnVector3 {
        self.xyz_cg
    }

    /// `GetDeltaXYZcg`: the CG shift of the last run, in.
    #[must_use]
    pub fn delta_xyz_cg(&self) -> ColumnVector3 {
        self.delta_xyz_cg
    }

    /// `GetJ`: the inertia tensor, body frame.
    #[must_use]
    pub fn j(&self) -> Matrix33 {
        self.j
    }

    /// `GetJinv`.
    #[must_use]
    pub fn jinv(&self) -> Matrix33 {
        self.jinv
    }

    /// The point masses, in load order (their property index).
    #[must_use]
    pub fn point_masses(&self) -> &[PointMass] {
        &self.point_masses
    }

    /// `SetEmptyWeight`.
    pub fn set_empty_weight(&mut self, weight: f64) {
        self.empty_weight = weight;
    }

    /// `SetBaseCG`.
    pub fn set_base_cg(&mut self, cg: ColumnVector3) {
        self.base_xyz_cg = cg;
        self.xyz_cg = cg;
    }

    /// `SetAircraftBaseInertias`.
    pub fn set_aircraft_base_inertias(&mut self, base_j: Matrix33) {
        self.base_j = base_j;
    }

    /// `GetTotalPointMassWeight`, lbs.
    #[must_use]
    pub fn total_point_mass_weight(&self) -> f64 {
        let mut total = 0.0;
        for pm in &self.point_masses {
            total += pm.weight;
        }
        total
    }

    /// `GetPointMassMoment`, lbs·in, structural frame.
    #[must_use]
    pub fn point_mass_moment(&self) -> ColumnVector3 {
        let mut moment = ColumnVector3::zero();
        for pm in &self.point_masses {
            moment += pm.weight * pm.location;
        }
        moment
    }

    /// `CalculatePMInertias`. Without point masses the sum is never
    /// formed and stays the zero it starts as.
    fn calculate_pm_inertias(&mut self) -> Matrix33 {
        if self.point_masses.is_empty() {
            return self.pm_j;
        }
        let mut pm_j = Matrix33::zero();
        for pm in &self.point_masses {
            pm_j += self.pointmass_inertia(LBTOSLUG * pm.weight, pm.location);
            pm_j += pm.inertia;
        }
        self.pm_j = pm_j;
        pm_j
    }

    /// `GetPointmassInertia`: the parallel-axis inertia of `mass_sl`
    /// slugs at `r` (structural frame, in) about the CG, body frame.
    #[must_use]
    pub fn pointmass_inertia(&self, mass_sl: f64, r: ColumnVector3) -> Matrix33 {
        let v = self.structural_to_body(r);
        let sv = mass_sl * v;
        let xx = sv[E_X] * v[E_X];
        let yy = sv[E_Y] * v[E_Y];
        let zz = sv[E_Z] * v[E_Z];
        let xy = -sv[E_X] * v[E_Y];
        let xz = -sv[E_X] * v[E_Z];
        let yz = -sv[E_Y] * v[E_Z];
        Matrix33::new(yy + zz, xy, xz, xy, xx + zz, yz, xz, yz, xx + yy)
    }

    /// `StructuralToBody`: structural (in, x aft) to body (ft, x forward)
    /// relative to the CG.
    #[must_use]
    pub fn structural_to_body(&self, r: ColumnVector3) -> ColumnVector3 {
        let x = &self.xyz_cg;
        let k = INCHTOFT;
        ColumnVector3::new(
            k * (x[E_X] - r[E_X]),
            k * (r[E_Y] - x[E_Y]),
            k * (x[E_Z] - r[E_Z]),
        )
    }
}

/// The inverse of a symmetric inertia tensor by cofactors (Stevens and
/// Lewis), as JSBSim computes it rather than by a general inverse.
fn inverse_inertia(j: &Matrix33) -> Matrix33 {
    let ixx = j[(E_X, E_X)];
    let iyy = j[(E_Y, E_Y)];
    let izz = j[(E_Z, E_Z)];
    let ixy = -j[(E_X, E_Y)];
    let ixz = -j[(E_X, E_Z)];
    let iyz = -j[(E_Y, E_Z)];

    let mut k1 = iyy * izz - iyz * iyz;
    let mut k2 = iyz * ixz + ixy * izz;
    let mut k3 = ixy * iyz + iyy * ixz;

    let denom = 1.0 / (ixx * k1 - ixy * k2 - ixz * k3);
    k1 *= denom;
    k2 *= denom;
    k3 *= denom;
    let k4 = (izz * ixx - ixz * ixz) * denom;
    let k5 = (ixy * ixz + iyz * ixx) * denom;
    let k6 = (ixx * iyy - ixy * ixy) * denom;

    Matrix33::new(k1, k2, k3, k2, k4, k5, k3, k5, k6)
}

crate::snapshot::visit_fields! {
    Inputs { gas_mass, tanks_weight, gas_moment, gas_inertia, tanks_moment, tank_inertia, wow }
}
crate::snapshot::visit_fields! {
    MassBalance {
        base, input, mass, weight, empty_weight, xyz_cg, j, jinv, pm_j, base_j, last_xyz_cg,
        delta_xyz_cg, delta_xyz_cg_body, base_xyz_cg, point_masses,
    }
}
