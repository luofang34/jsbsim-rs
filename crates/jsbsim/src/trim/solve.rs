// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Anthony K. Peden (apeden@earthlink.net)
// Derived from JSBSim src/initialization/FGTrim.cpp (solve, findInterval,
// checkLimits).
//
//! The one-axis root finder of `FGTrim`: a bracket of the control in which
//! the state changes sign, then regula falsi with relaxation on it.

use super::axis::TrimAxis;
use crate::Error;
use crate::fdm::Fdm;
use crate::initial_condition::InitialCondition;

/// The bracket and counters `FGTrim` keeps between its solver calls.
#[derive(Debug, Clone, PartialEq)]
pub struct Solver {
    /// `Nsub`: axis runs of the current axis in this cycle.
    pub nsub: u32,
    /// `max_sub_iterations`.
    pub max_sub_iterations: u32,
    /// `solutionDomain`: -1 for a root below the current control, 1 above,
    /// 0 for none.
    pub solution_domain: i32,
    /// `xlo`.
    pub xlo: f64,
    /// `xhi`.
    pub xhi: f64,
    /// `alo`: the state at `xlo`.
    pub alo: f64,
    /// `ahi`: the state at `xhi`.
    pub ahi: f64,
    /// `Debug`: 2 and above logs each solver step.
    pub debug: i32,
}

impl Default for Solver {
    fn default() -> Self {
        Self {
            nsub: 0,
            max_sub_iterations: 100,
            solution_domain: 0,
            xlo: 0.0,
            xhi: 0.0,
            alo: 0.0,
            ahi: 0.0,
            debug: 0,
        }
    }
}

/// `axis.SetControl(x); axis.Run(); axis.GetState()`.
fn state_at(
    axis: &mut TrimAxis,
    x: f64,
    fdm: &mut Fdm,
    ic: &mut InitialCondition,
) -> Result<f64, Error> {
    axis.set_control(x);
    axis.run(fdm, ic)?;
    Ok(axis.state(fdm))
}

impl Solver {
    /// `solve`: regula falsi on the bracket, relaxing the end that stays,
    /// until the axis is in tolerance, the step is below the axis' solver
    /// epsilon or the sub-iterations run out. True unless they ran out.
    pub fn solve(
        &mut self,
        axis: &mut TrimAxis,
        fdm: &mut Fdm,
        ic: &mut InitialCondition,
    ) -> Result<bool, Error> {
        const RELAX: f64 = 0.9;
        let eps = axis.solver_eps();
        let mut d = 1.0f64;
        let mut success = false;
        if self.solution_domain != 0 {
            let (mut x1, mut f1) = (self.xlo, self.alo);
            let (mut x3, mut f3) = (self.xhi, self.ahi);
            let d0 = (x3 - x1).abs();
            while !axis.in_tolerance(fdm) && d.abs() > eps && self.nsub < self.max_sub_iterations {
                self.nsub = self.nsub.wrapping_add(1);
                d = (x3 - x1) / d0;
                let x2 = x1 - d * d0 * f1 / (f3 - f1);
                let f2 = state_at(axis, x2, fdm, ic)?;
                if self.debug > 1 {
                    tracing::debug!(
                        "FGTrim::solve Nsub,x1,x2,x3: {}, {x1}, {x2}, {x3}\n                             {f1}, {f2}, {f3}",
                        self.nsub
                    );
                }
                if f1 * f2 <= 0.0 {
                    x3 = x2;
                    f3 = f2;
                    f1 *= RELAX;
                } else if f2 * f3 <= 0.0 {
                    x1 = x2;
                    f1 = f2;
                    f3 *= RELAX;
                }
            }
            if self.nsub < self.max_sub_iterations {
                success = true;
            }
        }
        Ok(success)
    }

    /// `findInterval`: widens a window around the current control, doubling
    /// the step from 2.5% of the control maximum, until the state changes
    /// sign across it; the bracket is then narrowed to the half that holds
    /// the root. False when the sub-iterations run out first.
    pub fn find_interval(
        &mut self,
        axis: &mut TrimAxis,
        fdm: &mut Fdm,
        ic: &mut InitialCondition,
    ) -> Result<bool, Error> {
        let mut found = false;
        let current_control = axis.control();
        let current_accel = axis.state(fdm);
        let (xmin, xmax) = (axis.control_min(), axis.control_max());
        let mut step = 0.025 * xmax.abs();
        (self.xlo, self.xhi) = (current_control, current_control);
        (self.alo, self.ahi) = (current_accel, current_accel);
        let (mut lastxlo, mut lastxhi) = (self.xlo, self.xhi);
        let (mut lastalo, mut lastahi) = (self.alo, self.ahi);
        loop {
            self.nsub = self.nsub.wrapping_add(1);
            step *= 2.0;
            self.xlo -= step;
            if self.xlo < xmin {
                self.xlo = xmin;
            }
            self.xhi += step;
            if self.xhi > xmax {
                self.xhi = xmax;
            }
            self.alo = state_at(axis, self.xlo, fdm, ic)?;
            self.ahi = state_at(axis, self.xhi, fdm, ic)?;
            // C++ `continue`s jump to the loop condition.
            if (self.ahi - self.alo).abs() > axis.tolerance() {
                if self.alo * self.ahi <= 0.0 {
                    found = true;
                    if self.alo * current_accel <= 0.0 {
                        self.solution_domain = -1;
                        self.xhi = lastxlo;
                        self.ahi = lastalo;
                    } else {
                        self.solution_domain = 1;
                        self.xlo = lastxhi;
                        self.alo = lastahi;
                    }
                }
                (lastxlo, lastxhi) = (self.xlo, self.xhi);
                (lastalo, lastahi) = (self.alo, self.ahi);
                let pinned = !found && self.xlo == xmin && self.xhi == xmax;
                if !pinned && self.debug > 1 {
                    tracing::debug!(
                        "FGTrim::findInterval: Nsub={} Lo= {} Hi= {} alo*ahi: {}",
                        self.nsub,
                        self.xlo,
                        self.xhi,
                        self.alo * self.ahi
                    );
                }
            }
            if found || self.nsub > self.max_sub_iterations {
                break;
            }
        }
        Ok(found)
    }

    /// `checkLimits`: the state at both ends of the control range; when it
    /// changes by more than the tolerance and changes sign between an end
    /// and the current control, that half becomes the bracket. The axis is
    /// then run at its current control again.
    pub fn check_limits(
        &mut self,
        axis: &mut TrimAxis,
        fdm: &mut Fdm,
        ic: &mut InitialCondition,
    ) -> Result<bool, Error> {
        let current_control = axis.control();
        let current_accel = axis.state(fdm);
        self.xlo = axis.control_min();
        self.xhi = axis.control_max();
        self.alo = state_at(axis, self.xlo, fdm, ic)?;
        self.ahi = state_at(axis, self.xhi, fdm, ic)?;
        if self.debug > 1 {
            tracing::debug!(
                "checkLimits() xlo,xhi,alo,ahi: {}, {}, {}, {}",
                self.xlo,
                self.xhi,
                self.alo,
                self.ahi
            );
        }
        self.solution_domain = 0;
        let mut solution_exists = false;
        if (self.ahi - self.alo).abs() > axis.tolerance() {
            if self.alo * current_accel <= 0.0 {
                solution_exists = true;
                self.solution_domain = -1;
                self.xhi = current_control;
                self.ahi = current_accel;
            } else if current_accel * self.ahi < 0.0 {
                solution_exists = true;
                self.solution_domain = 1;
                self.xlo = current_control;
                self.alo = current_accel;
            }
        }
        axis.set_control(current_control);
        axis.run(fdm, ic)?;
        Ok(solution_exists)
    }
}
