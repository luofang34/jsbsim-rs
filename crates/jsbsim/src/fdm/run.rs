// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.cpp (Run, LoadInputs, RunIC,
// Initialize, InitializeModels, Load*Constants, SetHoldDown).
//
//! Stepping the models in JSBSim's order, with each model's inputs loaded
//! from the others just before it runs.

use super::exec_bind::ExecRequest;
use super::{Fdm, Models};
use crate::Error;
use crate::math::{ColumnVector3, Location, Quaternion};
use crate::models::aircraft;

/// What the initial conditions give the models (`FGInitialCondition`'s
/// getters used by `SetInitialState`).
#[derive(Debug, Clone, Default)]
pub struct InitialState {
    /// Position.
    pub position: Location,
    /// Earth position angle, rad.
    pub epa: f64,
    /// Local-to-body orientation.
    pub orientation: Quaternion,
    /// Body velocity, ft/s.
    pub uvw: ColumnVector3,
    /// Body rates, rad/s.
    pub pqr: ColumnVector3,
    /// Steady wind, NED, ft/s.
    pub wind_ned: ColumnVector3,
}

/// JSBSim's model order (`eModels`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Model {
    Propagate,
    Input,
    Inertial,
    Atmosphere,
    Winds,
    Systems,
    MassBalance,
    Auxiliary,
    Propulsion,
    Aerodynamics,
    GroundReactions,
    ExternalReactions,
    BuoyantForces,
    Aircraft,
    Accelerations,
    Output,
}

const ORDER: [Model; 16] = [
    Model::Propagate,
    Model::Input,
    Model::Inertial,
    Model::Atmosphere,
    Model::Winds,
    Model::Systems,
    Model::MassBalance,
    Model::Auxiliary,
    Model::Propulsion,
    Model::Aerodynamics,
    Model::GroundReactions,
    Model::ExternalReactions,
    Model::BuoyantForces,
    Model::Aircraft,
    Model::Accelerations,
    Model::Output,
];

impl Models {
    /// `FGFDMExec::SetHoldDown`.
    pub fn set_hold_down(&mut self, hd: bool) {
        self.exec.hold_down = hd;
        self.accelerations.set_hold_down(hd);
        if hd {
            self.propagate.input.pqridot = self.accelerations.pqridot;
            self.propagate.input.uvwidot = self.accelerations.uvwidot;
        }
        self.propagate.set_hold_down(hd);
    }

    /// `LoadInputs(idx)`.
    fn load_inputs(&mut self, model: Model) {
        match model {
            Model::Propagate => {
                let p = &mut self.propagate.input;
                p.pqridot = self.accelerations.pqridot;
                p.uvwidot = self.accelerations.uvwidot;
                p.delta_t = self.exec.dt;
            }
            Model::Inertial => self.inertial.position = self.propagate.state.location.clone(),
            Model::Atmosphere => {
                let a = &mut self.atmosphere.input;
                a.altitude_asl = self.propagate.altitude_asl();
                a.geod_latitude_deg = self.propagate.state.location.geod_latitude_deg();
                a.longitude_deg = self.propagate.state.location.longitude_deg();
            }
            Model::Accelerations => self.load_accelerations_inputs(),
            _ => {}
        }
    }

    fn load_accelerations_inputs(&mut self) {
        let f = &self.propagate.frames;
        let a = &mut self.accelerations.input;
        a.j = self.mass_balance.j;
        a.jinv = self.mass_balance.jinv;
        a.ti2b = f.ti2b;
        a.tb2i = f.tb2i;
        a.tec2b = f.tec2b;
        a.tec2i = f.tec2i;
        a.moment = self.aircraft.moments;
        a.ground_moment = self.ground_reactions.moments;
        a.force = self.aircraft.forces;
        a.ground_force = self.ground_reactions.forces;
        a.grav_accel = self.inertial.grav_accel;
        a.pqri = self.propagate.state.pqri;
        a.pqr = self.propagate.state.pqr;
        a.uvw = self.propagate.state.uvw;
        a.inertial_position = self.propagate.state.inertial_position;
        a.delta_t = self.exec.dt;
        a.mass = self.mass_balance.mass;
        a.multipliers.clear();
        a.terrain_velocity = self.propagate.local_terrain_velocity;
        a.terrain_angular_vel = self.propagate.local_terrain_angular_velocity;
    }

    fn aircraft_inputs(&self) -> aircraft::Inputs {
        let fm = |m: &crate::models::forces::ForceModel| (m.forces, m.moments);
        aircraft::Inputs {
            aero: fm(&self.aerodynamics),
            propulsion: fm(&self.propulsion),
            ground: fm(&self.ground_reactions),
            external: fm(&self.external_reactions),
            buoyant: fm(&self.buoyant_forces),
        }
    }

    /// `InitModel` of `model`.
    fn init_model(&mut self, model: Model) {
        match model {
            Model::Propagate => self.propagate.init_model(&self.inertial),
            Model::Inertial => self.inertial.base.init(),
            Model::Aircraft => self.aircraft.init_model(),
            Model::Accelerations => self.accelerations.init_model(),
            Model::Atmosphere => self.atmosphere.base.init(),
            Model::Winds => self.winds.base.init(),
            Model::MassBalance => self.mass_balance.base.init(),
            Model::Auxiliary => self.auxiliary.base.init(),
            _ => {}
        }
    }
}

impl Fdm {
    /// `LoadPlanetConstants`.
    pub(super) fn load_planet_constants(&mut self) {
        let m = &mut self.models;
        m.propagate.input.omega_planet = m.inertial.omega_planet;
        m.accelerations.input.omega_planet = m.inertial.omega_planet;
        m.propagate.input.semi_major = m.inertial.a;
        m.propagate.input.semi_minor = m.inertial.b;
        m.propagate.input.gm = m.inertial.gm;
    }

    /// `InitializeModels`: every model but input and output.
    pub(super) fn initialize_models(&mut self) {
        for model in ORDER {
            if matches!(model, Model::Input | Model::Output) {
                continue;
            }
            self.models.load_inputs(model);
            self.models.init_model(model);
        }
    }

    /// `IncrTime`.
    fn incr_time(&mut self) {
        let e = &mut self.models.exec;
        if !e.holding && !e.integration_suspended() {
            e.sim_time += e.dt;
            self.models.inertial.time = e.sim_time;
            e.frame = e.frame.wrapping_add(1);
        }
    }

    /// `Run`: one frame. False once the simulation is to terminate.
    pub fn run(&mut self) -> Result<bool, Error> {
        self.incr_time();
        for model in ORDER {
            self.models.load_inputs(model);
            self.run_model(model)?;
        }
        Ok(!self.models.exec.terminate)
    }

    fn run_model(&mut self, model: Model) -> Result<(), Error> {
        let holding = self.models.exec.holding;
        let suspended = self.models.exec.integration_suspended();
        let m = &mut self.models;
        match model {
            Model::Propagate => m.propagate.run(holding, suspended, &m.inertial),
            Model::Inertial => m.inertial.run(holding),
            Model::Aircraft => {
                if !m.aircraft.base.skip() && !holding {
                    let input = m.aircraft_inputs();
                    m.aircraft.run(&input);
                }
            }
            Model::Accelerations => {
                let (hd, ts) = (m.exec.hold_down, m.exec.trim_status);
                m.accelerations.run(holding, hd, ts);
            }
            _ => {}
        }
        Ok(())
    }

    /// `Initialize(IC)`: the initial state into the models, then one frame.
    fn initialize(&mut self) -> Result<(), Error> {
        let ic = self.initial.clone();
        let m = &mut self.models;
        m.propagate.set_initial_state(
            &m.inertial,
            ic.position,
            ic.epa,
            ic.orientation,
            ic.uvw,
            ic.pqr,
        );
        m.winds.wind_ned = ic.wind_ned;
        self.run()?;
        Ok(())
    }

    /// `RunIC`: initialise from the initial conditions with integration
    /// suspended, run one frame, and seed the integrator histories.
    pub fn run_ic(&mut self) -> Result<(), Error> {
        let e = &mut self.models.exec;
        e.saved_dt = e.dt;
        e.dt = 0.0;
        self.initialize()?;
        self.run()?;
        self.models.propagate.initialize_derivatives();
        let e = &mut self.models.exec;
        e.dt = e.saved_dt;
        Ok(())
    }

    /// Sets the initial conditions `run_ic` applies.
    pub fn set_initial_state(&mut self, state: InitialState) {
        self.initial = state;
    }

    /// `Setdt`.
    pub fn set_dt(&mut self, dt: f64) {
        self.models.exec.dt = dt;
    }

    /// `ResetToInitialConditions(mode)`.
    pub fn reset_to_initial_conditions(&mut self, mode: i32) -> Result<(), Error> {
        self.initialize_models();
        self.models.exec.sim_time = 0.0;
        self.models.inertial.time = 0.0;
        if mode & 2 == 0 {
            self.run_ic()?;
        }
        Ok(())
    }

    /// Carries out what setters queued.
    pub(super) fn process_requests(&mut self) -> Result<(), Error> {
        while let Some(request) = self.models.exec.requests.pop() {
            match request {
                ExecRequest::Reset(mode) => self.reset_to_initial_conditions(mode)?,
                ExecRequest::Trim(_) => {
                    return Err(Error::Unsupported {
                        construct: "simulation/do_simple_trim (FGTrim)".to_owned(),
                        path: "property write".to_owned(),
                    });
                }
                ExecRequest::Linearize => {
                    return Err(Error::Unsupported {
                        construct: "simulation/do_linearization".to_owned(),
                        path: "property write".to_owned(),
                    });
                }
            }
        }
        Ok(())
    }
}
