// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2009 James Goppert.
// Derived from JSBSim src/math/FGStateSpace.{h,cpp}.
//
//! `FGStateSpace`: setting a vector of components re-initialises the
//! simulation from the initial conditions and runs it, with integration
//! suspended, until the states settle; the Jacobians are central
//! differences over such runs.

use std::f64::consts::PI;

use super::{Component, Matrix};
use crate::Error;
use crate::fdm::Fdm;

/// The perturbation of `FGStateSpace::linearize`.
const H: f64 = 1e-4;
/// Frames `run` takes at most for the states to settle.
const MAX_SETTLE: u32 = 1000;

/// The states, inputs and outputs of the state space.
pub struct StateSpace {
    x: Vec<Component>,
    u: Vec<Component>,
    y: Vec<Component>,
}

/// Which component vector.
#[derive(Clone, Copy)]
enum Which {
    X,
    U,
    Y,
}

impl StateSpace {
    pub(super) fn new(x: Vec<Component>, u: Vec<Component>, y: Vec<Component>) -> Self {
        Self { x, u, y }
    }

    fn vector(&self, which: Which) -> &[Component] {
        match which {
            Which::X => &self.x,
            Which::U => &self.u,
            Which::Y => &self.y,
        }
    }

    /// `run`: initialise from the initial conditions, start every
    /// engine's `InitRunning`, then run trimming frames with integration
    /// suspended until the sum of the states stops changing.
    fn run(&self, fdm: &mut Fdm) -> Result<(), Error> {
        fdm.initialize_from(fdm.models.ic.initial_state())?;
        for i in 0..fdm.models.propulsion.engines.len() {
            fdm.engine_init_running(i)?;
        }
        let mut cost = self.state_sum(fdm);
        for _ in 0..MAX_SETTLE {
            fdm.steady_state()?;
            fdm.models.exec.trim_status = true;
            fdm.models.output.base.enabled = false;
            let e = &mut fdm.models.exec;
            e.saved_dt = e.dt;
            e.dt = 0.0;
            fdm.run()?;
            fdm.models.exec.trim_status = false;
            fdm.models.output.base.enabled = true;
            let e = &mut fdm.models.exec;
            e.dt = e.saved_dt;
            let cost_new = self.state_sum(fdm);
            if (cost_new - cost).abs() < f64::EPSILON {
                break;
            }
            cost = cost_new;
        }
        Ok(())
    }

    fn state_sum(&self, fdm: &Fdm) -> f64 {
        let mut sum = 0.0;
        for c in &self.x {
            sum += c.get(fdm);
        }
        sum
    }

    /// `ComponentVector::set(vals)`: every component, then `run`.
    fn set_all(&self, fdm: &mut Fdm, which: Which, vals: &[f64]) -> Result<(), Error> {
        for (c, &v) in self.vector(which).iter().zip(vals) {
            c.set(fdm, v);
        }
        self.run(fdm)
    }

    /// `ComponentVector::set(i, val)`: one component, then `run`.
    fn set_one(&self, fdm: &mut Fdm, c: Component, val: f64) -> Result<(), Error> {
        c.set(fdm, val);
        self.run(fdm)
    }

    /// `getDeriv`: the component's own derivative, or JSBSim's default of
    /// one frame at 1/120 s, after which the states are set back and the
    /// time step and time restored.
    fn deriv(&self, fdm: &mut Fdm, c: Component) -> Result<f64, Error> {
        if let Some(d) = c.analytic_deriv(fdm) {
            return Ok(d);
        }
        let x0: Vec<f64> = self.x.iter().map(|x| x.get(fdm)).collect();
        let f0 = c.get(fdm);
        let (dt0, time0) = (fdm.models.exec.dt, fdm.models.exec.sim_time);
        fdm.set_dt(1. / 120.);
        fdm.models.output.base.enabled = false;
        fdm.run()?;
        let f1 = c.get(fdm);
        self.set_all(fdm, Which::X, &x0)?;
        let deriv = (f1 - f0) / fdm.models.exec.dt;
        fdm.set_dt(dt0);
        fdm.set_sim_time(time0);
        fdm.models.output.base.enabled = true;
        Ok(deriv)
    }

    fn value(&self, fdm: &mut Fdm, c: Component, derivative: bool) -> Result<f64, Error> {
        match derivative {
            true => self.deriv(fdm, c),
            false => Ok(c.get(fdm)),
        }
    }

    /// `numericalJacobian`: `J[i][j]`, the derivative of output `y[i]`
    /// (its time derivative with `derivative`) over input `x[j]`, from
    /// perturbations of ±`h` and ±2`h` about `x0`.
    fn jacobian(
        &self,
        fdm: &mut Fdm,
        (y, x): (Which, Which),
        x0: &[f64],
        derivative: bool,
    ) -> Result<Matrix, Error> {
        let (ny, nx) = (self.vector(y).len(), self.vector(x).len());
        let mut j = vec![vec![0.0; nx]; ny];
        for (iy, row) in j.iter_mut().enumerate() {
            for (ix, cell) in row.iter_mut().enumerate() {
                let (Some(&yc), Some(&xc)) = (self.vector(y).get(iy), self.vector(x).get(ix))
                else {
                    continue;
                };
                let mut f = [0.0; 4];
                for (slot, step) in f.iter_mut().zip([H, 2.0 * H, -H, -2.0 * H]) {
                    self.set_all(fdm, x, x0)?;
                    let v = xc.get(fdm);
                    self.set_one(fdm, xc, v + step)?;
                    *slot = self.value(fdm, yc, derivative)?;
                }
                let [f1, f2, fn1, fn2] = f;
                let (mut diff1, mut diff2) = (f1 - fn1, f2 - fn2);
                match xc.unit() {
                    "rad" => {
                        while diff1 > PI {
                            diff1 -= 2.0 * PI;
                        }
                        if diff1 < -PI {
                            diff1 += 2.0 * PI;
                        }
                        if diff2 > PI {
                            diff2 -= 2.0 * PI;
                        }
                        if diff2 < -PI {
                            diff2 += 2.0 * PI;
                        }
                    }
                    "deg" => {
                        if diff1 > 180.0 {
                            diff1 -= 360.0;
                        }
                        if diff1 < -180.0 {
                            diff1 += 360.0;
                        }
                        if diff2 > 180.0 {
                            diff2 -= 360.0;
                        }
                        if diff2 < -180.0 {
                            diff2 += 360.0;
                        }
                    }
                    _ => {}
                }
                // Third-order Taylor approximation (Stevens and Lewis, p. 203).
                *cell = (8.0 * diff1 - diff2) / (12.0 * H);
                self.set_all(fdm, x, x0)?;
            }
        }
        Ok(j)
    }

    /// `linearize`: A, B, C and D about `x0` and `u0`.
    pub(super) fn linearize(
        &mut self,
        fdm: &mut Fdm,
        x0: &[f64],
        u0: &[f64],
        _y0: &[f64],
    ) -> Result<[Matrix; 4], Error> {
        let a = self.jacobian(fdm, (Which::X, Which::X), x0, true)?;
        let b = self.jacobian(fdm, (Which::X, Which::U), u0, true)?;
        let c = self.jacobian(fdm, (Which::Y, Which::X), x0, false)?;
        let d = self.jacobian(fdm, (Which::Y, Which::U), u0, false)?;
        Ok([a, b, c, d])
    }
}
