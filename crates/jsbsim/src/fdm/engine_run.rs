// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGPropulsion.cpp (InitRunning,
// SetEngineRunning, GetSteadyState).
//
//! Starting engines and running them to a steady state.

use super::Fdm;
use crate::Error;
use crate::math::ColumnVector3;

impl Fdm {
    /// `FGPropulsion::InitRunning(n)`: engine `n`, or every engine for a
    /// negative `n`, set running, then the steady state.
    pub(super) fn init_running(&mut self, n: i32) -> Result<(), Error> {
        let count = self.models.propulsion.engines.len();
        match usize::try_from(n) {
            Ok(i) if i >= count => {
                return Err(Error::Model {
                    message: "Tried to initialize a non-existent engine!".to_owned(),
                });
            }
            Ok(i) => self.set_engine_running(i),
            Err(_) => (0..count).for_each(|i| self.set_engine_running(i)),
        }
        self.steady_state()?;
        Ok(())
    }

    /// `SetEngineRunning`: full throttle and mixture in the engine inputs,
    /// full mixture in the FCS, then the engine's `InitRunning`.
    fn set_engine_running(&mut self, i: usize) {
        let input = &mut self.models.propulsion.input;
        for v in [
            &mut input.throttle_cmd,
            &mut input.throttle_pos,
            &mut input.mixture_cmd,
            &mut input.mixture_pos,
        ] {
            if let Some(x) = v.get_mut(i) {
                *x = 1.0;
            }
        }
        let n = i32::try_from(i).unwrap_or(i32::MAX);
        let throttles = &mut self.models.fcs.throttles;
        throttles.set_mixture_pos(n, 1.0);
        throttles.set_mixture_cmd(n, 1.0);
        let p = &mut self.models.propulsion;
        if let Some(e) = p.engines.get_mut(i) {
            e.kind
                .init_running(&mut e.base, &mut e.thruster, &mut p.input);
        }
    }

    /// `GetSteadyState`: each engine calculated with a half-second step
    /// until its thrust settles, in trim mode, and its force summed; true
    /// when the model is not due to run.
    pub fn steady_state(&mut self) -> Result<bool, Error> {
        let (trim_mode, time_step) = (self.models.exec.trim_status, self.models.exec.dt);
        let p = &mut self.models.propulsion;
        p.forces = ColumnVector3::zero();
        p.moments = ColumnVector3::zero();
        if p.base.skip() {
            return Ok(true);
        }
        self.models.exec.trim_status = true;
        self.models.propulsion.input.total_delta_t = 0.5;
        // The thrusts carry over from one engine to the next, as in JSBSim.
        let mut current = 0.0;
        for i in 0..self.models.propulsion.engines.len() {
            let (mut steady, mut steady_count, mut j) = (false, 0u32, 0u32);
            while !steady && j < 6000 {
                self.calculate_engine(i)?;
                let last = current;
                current = self
                    .models
                    .propulsion
                    .engines
                    .get(i)
                    .map_or(0.0, |e| e.thruster.base.thrust);
                if (last - current).abs() < 0.0001 {
                    steady_count = steady_count.wrapping_add(1);
                    steady = steady_count > 120;
                } else {
                    steady_count = 0;
                }
                j = j.wrapping_add(1);
            }
            self.sum_engine_forces(i);
        }
        self.models.exec.trim_status = trim_mode;
        self.models.propulsion.input.total_delta_t = time_step;
        Ok(false)
    }
}
