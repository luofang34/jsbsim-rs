// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Copyright (C) 2009 James Goppert.
// Derived from JSBSim src/FGFDMExec.cpp (DoLinearization) and
// src/initialization/FGLinearization.{h,cpp}.
//
//! The executive's linearization entry point, which
//! `simulation/do_linearization` calls.

use super::Fdm;
use crate::Error;
use crate::linearization::{Component, Linearization, StateSpace};
use crate::models::propulsion::families::ThrusterKind;

/// The states `FGLinearization` linearizes over: speed and the
/// longitudinal states, a propeller's speed per engine (up to four) when
/// the first engine drives a propeller, the lateral states and the
/// position.
fn states(fdm: &Fdm) -> Result<Vec<Component>, Error> {
    let engines = &fdm.models.propulsion.engines;
    let Some(first) = engines.first() else {
        // `GetEngine(0)` of an aircraft without engines reads past the end
        // of JSBSim's vector.
        return Err(Error::model(
            "FGLinearization needs an aircraft with an engine",
        ));
    };
    let mut x = vec![
        Component::Vt,
        Component::Alpha,
        Component::Theta,
        Component::Q,
    ];
    if matches!(first.thruster.kind, ThrusterKind::Propeller(_)) {
        x.extend(Component::RPM.into_iter().take(engines.len()));
        if engines.len() > 4 {
            tracing::error!("More than 4 engines not currently handled");
        }
    }
    x.extend([
        Component::Beta,
        Component::Phi,
        Component::P,
        Component::Psi,
        Component::R,
        Component::Latitude,
        Component::Longitude,
        Component::Alt,
    ]);
    Ok(x)
}

impl Fdm {
    /// `FGLinearization(fdm)`: linearizes about the current state, with
    /// integration suspended. As in JSBSim the time step is left at zero
    /// afterwards (the state space's runs suspend integration again, which
    /// saves the suspended step); [`do_linearization`](Self::do_linearization)
    /// restores it.
    pub(crate) fn linearize(&mut self) -> Result<Linearization, Error> {
        let x = states(self)?;
        let u = vec![
            Component::ThrottleCmd,
            Component::DaCmd,
            Component::DeCmd,
            Component::DrCmd,
        ];
        let y = x.clone();
        let x0: Vec<f64> = x.iter().map(|c| c.get(self)).collect();
        let u0: Vec<f64> = u.iter().map(|c| c.get(self)).collect();
        let y0 = x0.clone();
        let e = &mut self.models.exec;
        e.saved_dt = e.dt;
        e.dt = 0.0;
        let ss = StateSpace::new(x.clone(), u.clone(), y.clone());
        let [a, b, c, d] = ss.linearize(self, &x0, &u0, &y0)?;
        let e = &mut self.models.exec;
        e.dt = e.saved_dt;
        Ok(Linearization {
            aircraft_name: self.models.aircraft.name.clone(),
            x,
            u,
            y,
            x0,
            u0,
            y0,
            a,
            b,
            c,
            d,
        })
    }

    /// `DoLinearization`: the state-space model about the current state,
    /// usually a trimmed one, which writing `simulation/do_linearization`
    /// computes too. JSBSim writes the result to `<aircraft>_lin.sce`; here
    /// it is returned, and kept for
    /// [`last_linearization`](Self::last_linearization).
    ///
    /// Each perturbation re-initialises the simulation from the initial
    /// conditions with every engine running, so the initial conditions end
    /// up holding the operating point and the aircraft is left re-initialised
    /// there. The time step and the simulation time come back as they were.
    ///
    /// # Errors
    ///
    /// [`Error::Model`] for an aircraft without an engine, which
    /// `FGLinearization` cannot handle, and the errors of
    /// [`run`](Self::run) from the frames it runs. After a failed frame the
    /// time step is back, the rest of the simulation is where that frame
    /// stopped, and [`last_linearization`](Self::last_linearization) still
    /// holds the earlier result.
    pub fn do_linearization(&mut self) -> Result<&Linearization, Error> {
        let dt0 = self.models.exec.dt;
        let result = self.linearize();
        self.set_dt(dt0);
        let lin = result?;
        tracing::info!(
            "linearized {}: the core writes no {}_lin.sce",
            lin.aircraft_name,
            lin.aircraft_name
        );
        Ok(self.last_linearization.insert(lin))
    }

    /// What the last successful [`do_linearization`](Self::do_linearization)
    /// or write of `simulation/do_linearization` computed; `None` before the
    /// first.
    #[must_use]
    pub fn last_linearization(&self) -> Option<&Linearization> {
        self.last_linearization.as_ref()
    }
}
