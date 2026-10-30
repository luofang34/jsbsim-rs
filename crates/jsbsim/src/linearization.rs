// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2009 James Goppert.
// Derived from JSBSim src/initialization/FGLinearization.{h,cpp}.
//
//! Linearization: the state-space model `dx/dt = A x + B u`,
//! `y = C x + D u` of the aircraft about its current (trimmed) state,
//! by central differences through the simulation (`FGStateSpace`).
//!
//! JSBSim writes the matrices to a Scicoslab file; the core does no file
//! I/O and returns them instead. Writing the file changes nothing a
//! simulation can observe.

mod component;
mod state_space;
#[cfg(test)]
mod tests;

pub use component::Component;

use crate::Error;
use crate::fdm::Fdm;
use crate::models::propulsion::families::ThrusterKind;

/// A row-major matrix, as JSBSim's `std::vector<std::vector<double>>`.
pub type Matrix = Vec<Vec<f64>>;

/// What `FGLinearization` computes.
#[derive(Debug, Clone, PartialEq)]
pub struct Linearization {
    /// The aircraft's name.
    pub aircraft_name: String,
    /// The states, in order.
    pub x: Vec<Component>,
    /// The inputs, in order.
    pub u: Vec<Component>,
    /// The outputs (the states: state feedback), in order.
    pub y: Vec<Component>,
    /// The states at the operating point.
    pub x0: Vec<f64>,
    /// The inputs at the operating point.
    pub u0: Vec<f64>,
    /// The outputs at the operating point.
    pub y0: Vec<f64>,
    /// `A = d(dx/dt)/dx`, one row per state.
    pub a: Matrix,
    /// `B = d(dx/dt)/du`, one row per state.
    pub b: Matrix,
    /// `C = dy/dx`, one row per output.
    pub c: Matrix,
    /// `D = dy/du`, one row per output.
    pub d: Matrix,
}

impl Linearization {
    /// `x_names`.
    #[must_use]
    pub fn x_names(&self) -> Vec<&'static str> {
        self.x.iter().map(|c| c.name()).collect()
    }

    /// `u_names`.
    #[must_use]
    pub fn u_names(&self) -> Vec<&'static str> {
        self.u.iter().map(|c| c.name()).collect()
    }

    /// `y_names`.
    #[must_use]
    pub fn y_names(&self) -> Vec<&'static str> {
        self.y.iter().map(|c| c.name()).collect()
    }

    /// `x_units`.
    #[must_use]
    pub fn x_units(&self) -> Vec<&'static str> {
        self.x.iter().map(|c| c.unit()).collect()
    }

    /// `u_units`.
    #[must_use]
    pub fn u_units(&self) -> Vec<&'static str> {
        self.u.iter().map(|c| c.unit()).collect()
    }

    /// `y_units`.
    #[must_use]
    pub fn y_units(&self) -> Vec<&'static str> {
        self.y.iter().map(|c| c.unit()).collect()
    }
}

/// The states `FGLinearization` linearizes over: speed and the
/// longitudinal states, a propeller's speed per engine (up to four) when
/// the first engine drives a propeller, the lateral states and the
/// position.
fn states(fdm: &Fdm) -> Result<Vec<Component>, Error> {
    let engines = &fdm.models.propulsion.engines;
    let Some(first) = engines.first() else {
        // `GetEngine(0)` of an aircraft without engines reads past the end
        // of JSBSim's vector.
        return Err(Error::Model {
            message: "FGLinearization needs an aircraft with an engine".to_owned(),
        });
    };
    let mut x = vec![
        Component::Vt,
        Component::Alpha,
        Component::Theta,
        Component::Q,
    ];
    if matches!(first.thruster.kind, ThrusterKind::Propeller(_)) {
        x.extend((0..engines.len().min(4)).map(Component::Rpm));
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
    /// saves the suspended step); `simulation/do_linearization` restores
    /// it.
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
        let ss = state_space::StateSpace::new(x.clone(), u.clone(), y.clone());
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

    /// `DoLinearization`: [`linearize`](Self::linearize), then the time
    /// step restored. JSBSim writes the result to `<aircraft>_lin.sce`;
    /// here it is kept for [`last_linearization`](Self::last_linearization).
    pub(crate) fn do_linearization(&mut self) -> Result<(), Error> {
        let dt0 = self.models.exec.dt;
        let lin = self.linearize()?;
        tracing::info!(
            "linearized {}: the core writes no {}_lin.sce",
            lin.aircraft_name,
            lin.aircraft_name
        );
        self.last_linearization = Some(lin);
        self.set_dt(dt0);
        Ok(())
    }

    /// What the last `simulation/do_linearization` computed.
    #[must_use]
    #[cfg(any(test, feature = "__oracle"))]
    pub(crate) fn last_linearization(&self) -> Option<&Linearization> {
        self.last_linearization.as_ref()
    }
}
