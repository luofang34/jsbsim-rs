// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGMassBalance.cpp (bind, PointMass::bind,
// GetMassPropertiesReport).
//
//! Properties of `FGMassBalance`, in JSBSim's binding order.

use super::MassBalance;
use crate::Error;
use crate::fdm::{Binder, Kind};
use crate::math::{E_X, E_Y, E_Z};

/// `FGMassBalance::bind`. `inertia/ixz-slugs_ft2` is JSBSim's `GetIxz`,
/// which, unlike `GetIxy` and `GetIyz`, does not negate the tensor entry.
pub fn bind(b: &mut Binder<'_>) -> Result<(), Error> {
    b.getter("inertia/mass-slugs", |m| m.mass_balance.mass)?;
    b.getter("inertia/weight-lbs", |m| m.mass_balance.weight)?;
    b.getter("inertia/empty-weight-lbs", |m| m.mass_balance.empty_weight)?;
    for (name, i) in [("x", E_X), ("y", E_Y), ("z", E_Z)] {
        b.getter(&format!("inertia/cg-{name}-in"), move |m| {
            m.mass_balance.xyz_cg[i]
        })?;
    }
    b.getter("inertia/ixx-slugs_ft2", |m| m.mass_balance.j[(E_X, E_X)])?;
    b.getter("inertia/iyy-slugs_ft2", |m| m.mass_balance.j[(E_Y, E_Y)])?;
    b.getter("inertia/izz-slugs_ft2", |m| m.mass_balance.j[(E_Z, E_Z)])?;
    b.getter("inertia/ixy-slugs_ft2", |m| -m.mass_balance.j[(E_X, E_Y)])?;
    b.getter("inertia/ixz-slugs_ft2", |m| m.mass_balance.j[(E_X, E_Z)])?;
    b.getter("inertia/iyz-slugs_ft2", |m| -m.mass_balance.j[(E_Y, E_Z)])?;
    b.setter_only("inertia/print-mass-properties", Kind::Int, |m, _| {
        let tanks = m.propulsion.tank_report(&m.mass_balance);
        m.mass_balance.mass_properties_report(&tanks);
    })?;
    Ok(())
}

/// `PointMass::bind`: the weight and location of point mass `num`.
pub(super) fn bind_point_mass(b: &mut Binder<'_>, num: usize) -> Result<(), Error> {
    b.property(
        &format!("inertia/pointmass-weight-lbs[{num}]"),
        Kind::Double,
        move |m| {
            m.mass_balance
                .point_masses
                .get(num)
                .map_or(0.0, |p| p.weight)
        },
        move |m, v| {
            if let Some(p) = m.mass_balance.point_masses.get_mut(num) {
                p.set_weight(v);
            }
        },
    )?;
    for (name, axis) in [("X", E_X), ("Y", E_Y), ("Z", E_Z)] {
        b.property(
            &format!("inertia/pointmass-location-{name}-inches[{num}]"),
            Kind::Double,
            move |m| {
                m.mass_balance
                    .point_masses
                    .get(num)
                    .map_or(0.0, |p| p.location[axis])
            },
            move |m, v| {
                if let Some(p) = m.mass_balance.point_masses.get_mut(num) {
                    p.set_location(axis, v);
                }
            },
        )?;
    }
    Ok(())
}

impl MassBalance {
    /// `GetMassPropertiesReport`: the base vehicle, each point mass, the
    /// propulsion's `tank_report` and the total (lb, in, slug·ft^2).
    pub fn mass_properties_report(&self, tank_report: &str) {
        let row = |label: &str, w: f64, l: [f64; 3], j: &crate::math::Matrix33| {
            format!(
                "{label:<34}{w:>12.1}{:>8.1}{:>8.1}{:>8.1}{:>12.1}{:>12.1}{:>12.1}{:>12.1}{:>12.1}{:>12.1}",
                l[0],
                l[1],
                l[2],
                j[(E_X, E_X)],
                j[(E_Y, E_Y)],
                j[(E_Z, E_Z)],
                j[(E_X, E_Y)],
                j[(E_X, E_Z)],
                j[(E_Y, E_Z)]
            )
        };
        let v = |c: crate::math::ColumnVector3| [c[E_X], c[E_Y], c[E_Z]];
        let mut out =
            String::from("\n  Mass Properties Report (English units: lb, in, slug-ft^2)\n");
        out.push_str(&format!(
            "{:34}      Mass    CG-X    CG-Y    CG-Z         Ixx         Iyy         Izz         Ixy         Ixz         Iyz\n",
            ""
        ));
        out.push_str(&row(
            "    Base Vehicle ",
            self.empty_weight,
            v(self.base_xyz_cg),
            &self.base_j,
        ));
        out.push('\n');
        for (i, pm) in self.point_masses.iter().enumerate() {
            let label = format!("{i:<4}{:<30}", pm.name);
            out.push_str(&row(&label, pm.weight, v(pm.location), &pm.inertia));
            out.push('\n');
        }
        if !tank_report.is_empty() {
            out.push_str(&format!("\n    {:<34}\n{tank_report}", "Tanks Content"));
        }
        out.push_str(&row("    Total: ", self.weight, v(self.xyz_cg), &self.j));
        tracing::info!("{out}");
    }
}
