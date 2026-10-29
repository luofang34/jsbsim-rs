// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.cpp (Run, LoadInputs, RunIC,
// Initialize, InitializeModels, Load*Constants, SetHoldDown).
//
//! Stepping the models in JSBSim's order, with each model's inputs loaded
//! from the others just before it runs.

use super::exec_bind::ExecRequest;
use super::order::{Model, ORDER};
use super::{Fdm, Models};
use crate::Error;
use crate::math::{ColumnVector3, E_PHI, E_THT, Location, Quaternion};
use crate::models::aircraft;
use crate::models::atmosphere::{AtmosphereModel, Overrides};
use crate::properties::NodeId;
use crate::units::FPSTOKTS;

/// What the initial conditions give the models: the `FGInitialCondition`
/// getters that `FGFDMExec::Initialize` hands to `SetInitialState`.
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
    pub(super) fn load_inputs(&mut self, model: Model) {
        match model {
            Model::Propagate => {
                let p = &mut self.propagate.input;
                p.pqridot = self.accelerations.pqridot;
                p.uvwidot = self.accelerations.uvwidot;
                p.delta_t = self.exec.dt;
            }
            Model::Inertial => self.inertial.position = self.propagate.state.location,
            Model::Atmosphere => {
                let a = &mut self.atmosphere.input;
                a.altitude_asl = self.propagate.altitude_asl();
                a.geod_latitude_deg = self.propagate.state.location.geod_latitude_deg();
                a.longitude_deg = self.propagate.state.location.longitude_deg();
            }
            Model::Winds => {
                let w = &mut self.winds.input;
                w.altitude_asl = self.propagate.altitude_asl();
                w.distance_agl = self.propagate.distance_agl(&self.inertial);
                w.tl2b = self.propagate.frames.tl2b;
                w.tw2b = self.auxiliary.tw2b;
                w.uvw = self.propagate.state.uvw;
                w.total_delta_t = self.exec.dt * f64::from(self.winds.base.rate);
            }
            Model::Aerodynamics => {
                let x = &self.auxiliary;
                let a = &mut self.aerodynamics.input;
                a.alpha = x.alpha;
                a.beta = x.beta;
                a.qbar = x.qbar;
                a.vt = x.vt;
                a.tb2w = x.tb2w;
                a.tw2b = x.tw2b;
                a.rp_body = self.mass_balance.structural_to_body(self.aircraft.xyz_rp);
            }
            Model::GroundReactions => self.load_ground_reactions_inputs(),
            Model::BuoyantForces => {
                let b = &mut self.buoyant_forces.input;
                b.density = self.atmosphere.density;
                b.pressure = self.atmosphere.pressure;
                b.temperature = self.atmosphere.temperature;
                b.gravity = self.inertial.grav_accel.magnitude();
            }
            Model::MassBalance => self.load_mass_balance_inputs(),
            Model::Propulsion => self.load_propulsion_inputs(),
            Model::Accelerations => self.load_accelerations_inputs(),
            Model::Auxiliary => self.load_auxiliary_inputs(),
            _ => {}
        }
    }

    fn load_ground_reactions_inputs(&mut self) {
        let wow = self.ground_reactions.wow();
        let (p, f) = (&self.propagate, &self.propagate.frames);
        let g = &mut self.ground_reactions.input;
        g.vground = self.auxiliary.airspeed.vground;
        g.vcalibrated_kts = self.auxiliary.airspeed.vcas * FPSTOKTS;
        g.temperature = self.atmosphere.temperature;
        g.brake_pos = self.fcs.brake_pos;
        g.takeoff_throttle = self
            .fcs
            .throttles
            .throttle_pos
            .first()
            .is_some_and(|t| *t > 0.90);
        g.fcs_gear_pos = self.fcs.gear_pos;
        g.empty_weight = self.mass_balance.empty_weight;
        g.tb2l = f.tb2l;
        g.tec2l = f.tec2l;
        g.tec2b = f.tec2b;
        g.pqr = p.state.pqr;
        g.uvw = p.state.uvw;
        g.distance_agl = p.distance_agl(&self.inertial);
        g.distance_asl = p.altitude_asl();
        g.total_delta_t = self.exec.dt * f64::from(self.ground_reactions.base.rate);
        g.wow = wow;
        g.location.clone_from(&p.state.location);
        g.xyz_cg = self.mass_balance.xyz_cg;
    }

    fn load_mass_balance_inputs(&mut self) {
        let tank_inertia = self.propulsion.calculate_tank_inertias(&self.mass_balance);
        let i = &mut self.mass_balance.input;
        i.gas_inertia = self.buoyant_forces.gas_mass_inertia();
        i.gas_mass = self.buoyant_forces.gas_mass();
        i.gas_moment = self.buoyant_forces.gas_mass_moment();
        i.tanks_weight = self.propulsion.tanks_weight();
        i.tanks_moment = self.propulsion.tanks_moment();
        i.tank_inertia = tank_inertia;
        i.wow = self.ground_reactions.wow();
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
        self.ground_reactions.load_multipliers(&mut a.multipliers);
        a.terrain_velocity = self.propagate.local_terrain_velocity;
        a.terrain_angular_vel = self.propagate.local_terrain_angular_velocity;
    }

    fn load_auxiliary_inputs(&mut self) {
        let (p, at, ac) = (&self.propagate, &self.atmosphere, &self.accelerations);
        let a = &mut self.auxiliary.input;
        a.air = crate::models::auxiliary::Air {
            pressure: at.pressure,
            density: at.density,
            temperature: at.temperature,
            sound_speed: at.sound_speed,
            kinematic_viscosity: at.kinematic_viscosity,
        };
        a.distance_agl = p.distance_agl(&self.inertial);
        a.mass = self.mass_balance.mass;
        a.tl2b = p.frames.tl2b;
        a.tb2l = p.frames.tb2l;
        a.pqr = p.state.pqr;
        a.pqri = p.state.pqri;
        a.pqridot = ac.pqridot;
        a.uvw = p.state.uvw;
        a.uvwdot = ac.uvwdot;
        a.vel = p.vel;
        a.body_accel = ac.body_accel;
        a.to_eye_pt = self.mass_balance.structural_to_body(self.aircraft.xyz_ep);
        a.vrp_body = self.mass_balance.structural_to_body(self.aircraft.xyz_vrp);
        a.rp_body = self.mass_balance.structural_to_body(self.aircraft.xyz_rp);
        a.fw = self.aerodynamics.state.fw;
        a.location = p.state.location;
        let q = &p.state.q_attitude_local;
        a.cos_tht = q.cos_euler(E_THT);
        a.sin_tht = q.sin_euler(E_THT);
        a.cos_phi = q.cos_euler(E_PHI);
        a.sin_phi = q.sin_euler(E_PHI);
        a.total_wind_ned = self.winds.total_wind_ned;
        a.turb_pqr = self.winds.turb_pqr;
    }

    fn aircraft_inputs(&self) -> aircraft::Inputs {
        aircraft::Inputs {
            aero: (
                self.aerodynamics.state.forces,
                self.aerodynamics.state.moments,
            ),
            propulsion: (self.propulsion.forces, self.propulsion.moments),
            ground: (self.ground_reactions.forces, self.ground_reactions.moments),
            external: (
                self.external_reactions.total_forces,
                self.external_reactions.total_moments,
            ),
            buoyant: (self.buoyant_forces.forces, self.buoyant_forces.moments),
        }
    }

    /// `InitModel` of `model`.
    fn init_model(&mut self, model: Model) {
        match model {
            Model::Propagate => self.propagate.init_model(&self.inertial),
            Model::Inertial => self.inertial.base.init(),
            Model::Aircraft => self.aircraft.init_model(),
            Model::Accelerations => self.accelerations.init_model(),
            Model::Winds => self.winds.init_model(),
            Model::MassBalance => self.mass_balance.init_model(),
            Model::Auxiliary => self.auxiliary.init_model(),
            Model::Systems => self.fcs.init_model(),
            Model::Aerodynamics => self.aerodynamics.init_model(),
            Model::ExternalReactions => self.external_reactions.init_model(),
            Model::GroundReactions => self.ground_reactions.init_model(),
            Model::Propulsion => self.propulsion.init_model(),
            Model::BuoyantForces => self.buoyant_forces.init_model(),
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
        let c = &mut m.auxiliary.input.constants;
        c.standard_gravity = crate::models::inertial::STANDARD_GRAVITY;
        c.std_day_sl_soundspeed = m.atmosphere.std_day_sl_soundspeed;
    }

    /// `InitializeModels`: every model but input and output.
    pub(super) fn initialize_models(&mut self) -> Result<(), Error> {
        for model in ORDER {
            if matches!(model, Model::Input | Model::Output) {
                continue;
            }
            self.models.load_inputs(model);
            if model == Model::Atmosphere {
                self.init_atmosphere()?;
                // Only `FGAtmosphere::InitModel` chains to
                // `FGModel::InitModel`; the standard atmosphere's does not.
                if self.models.atmosphere.model != AtmosphereModel::Mars {
                    continue;
                }
            } else {
                self.models.init_model(model);
            }
            self.reset_local_properties(model)?;
        }
        Ok(())
    }

    /// `Atmosphere->InitModel()`.
    pub fn init_atmosphere(&mut self) -> Result<(), Error> {
        let overrides = self.atmosphere_overrides()?;
        self.models.atmosphere.init_model(&overrides);
        Ok(())
    }

    /// `Atmosphere->Run(holding)` on the inputs already loaded: the
    /// override properties are read only when the model calculates.
    pub fn run_atmosphere(&mut self, holding: bool) -> Result<(), Error> {
        if self.models.atmosphere.base.skip() || holding {
            return Ok(());
        }
        let overrides = self.atmosphere_overrides()?;
        let altitude = self.models.atmosphere.input.altitude_asl;
        self.models.atmosphere.calculate(altitude, &overrides);
        Ok(())
    }

    /// The `atmosphere/override/*` properties found so far, which only
    /// `FGAtmosphere::Calculate` reads.
    fn atmosphere_overrides(&mut self) -> Result<Overrides, Error> {
        if self.models.atmosphere.model != AtmosphereModel::Standard {
            return Ok(Overrides::default());
        }
        let nodes = &mut self.models.atmosphere.override_nodes;
        nodes.resolve(&self.env.tree);
        let nodes = *nodes;
        let mut read = |node: Option<NodeId>| node.map(|n| self.read_node(n)).transpose();
        Ok(Overrides {
            temperature: read(nodes.temperature)?,
            pressure: read(nodes.pressure)?,
            density: read(nodes.density)?,
        })
    }

    /// `LoadInputs` of every model.
    pub(super) fn load_all_inputs(&mut self) {
        for model in ORDER {
            self.models.load_inputs(model);
        }
    }

    /// `IncrTime`.
    fn incr_time(&mut self) {
        let e = &mut self.models.exec;
        if !e.holding && !e.integration_suspended() {
            e.sim_time += e.dt;
            self.models.inertial.time = e.sim_time;
            self.models.atmosphere.sim_time = e.sim_time;
            e.frame = e.frame.wrapping_add(1);
        }
    }

    /// `Run`: one frame. False once the simulation is to terminate.
    pub fn run(&mut self) -> Result<bool, Error> {
        self.incr_time();
        let mut success = true;
        if self.script.is_some() && !self.models.exec.integration_suspended() {
            success = self.run_script()?;
        }
        for model in ORDER {
            self.models.load_inputs(model);
            self.run_model(model)?;
        }
        Ok(success && !self.models.exec.terminate)
    }

    fn run_model(&mut self, model: Model) -> Result<(), Error> {
        let holding = self.models.exec.holding;
        if model == Model::Atmosphere {
            return self.run_atmosphere(holding);
        }
        let suspended = self.models.exec.integration_suspended();
        let m = &mut self.models;
        match model {
            Model::Propagate => m.propagate.run(holding, suspended, &m.inertial),
            Model::Inertial => m.inertial.run(holding),
            Model::Winds => m.winds.run(holding, &m.exec.rng, m.exec.sim_time),
            Model::MassBalance => {
                if !m.mass_balance.base.skip() && !holding {
                    self.run_model_functions(model, true)?;
                    let m = &mut self.models;
                    m.mass_balance.run_frame(m.exec.hold_down, &mut m.propagate);
                    self.run_model_functions(model, false)?;
                }
            }
            Model::Aircraft => {
                if !m.aircraft.base.skip() && !holding {
                    self.run_model_functions(model, true)?;
                    let input = self.models.aircraft_inputs();
                    self.models.aircraft.run(&input);
                    self.run_model_functions(model, false)?;
                }
            }
            Model::Systems => self.run_fcs()?,
            Model::Aerodynamics => self.run_aerodynamics()?,
            Model::GroundReactions => self.run_ground_reactions()?,
            Model::ExternalReactions => self.run_external_reactions()?,
            Model::Propulsion => self.run_propulsion()?,
            Model::BuoyantForces => self.run_buoyant_forces()?,
            Model::Accelerations => {
                let (hd, ts) = (m.exec.hold_down, m.exec.trim_status);
                m.accelerations.run(holding, hd, ts);
                let solved = &m.accelerations.input.multipliers;
                m.ground_reactions.store_multipliers(solved);
            }
            Model::Auxiliary => m.auxiliary.run(holding),
            Model::Output => self.run_output(holding)?,
            _ => {}
        }
        Ok(())
    }

    /// `Initialize(IC)`: the initial state into the models, then one frame.
    fn initialize(&mut self) -> Result<(), Error> {
        self.initialize_from(self.models.ic.initial_state())
    }

    /// `Initialize(FGIC)` from the state an initial condition gives.
    pub fn initialize_from(&mut self, ic: InitialState) -> Result<(), Error> {
        let m = &mut self.models;
        m.propagate.set_initial_state(
            &m.inertial,
            ic.position,
            ic.epa,
            ic.orientation,
            ic.uvw,
            ic.pqr,
        );
        m.winds.set_wind_ned(ic.wind_ned);
        let (a, b) = (m.inertial.a, m.inertial.b);
        m.auxiliary.set_initial_state(&ic.position, a, b);
        self.run()?;
        Ok(())
    }

    /// `RunIC`: initialise from the initial conditions with integration
    /// suspended, run one frame, seed the integrator histories, then start
    /// the engines the initial conditions say are running.
    pub fn run_ic(&mut self) -> Result<(), Error> {
        let e = &mut self.models.exec;
        e.saved_dt = e.dt;
        e.dt = 0.0;
        self.initialize()?;
        self.models.output.init_model();
        self.run()?;
        self.models.propagate.initialize_derivatives();
        let e = &mut self.models.exec;
        e.dt = e.saved_dt;
        for n in 0..self.models.propulsion.engines.len() {
            if self.models.ic.is_engine_running(n) {
                self.init_running(i32::try_from(n).unwrap_or(i32::MAX))?;
            }
        }
        Ok(())
    }

    /// `Setdt`.
    pub fn set_dt(&mut self, dt: f64) {
        self.models.exec.dt = dt;
    }

    /// `ResetToInitialConditions(mode)`.
    pub fn reset_to_initial_conditions(&mut self, mode: i32) -> Result<(), Error> {
        self.initialize_models()?;
        if self.script.is_some() {
            self.reset_events()?;
        } else {
            self.set_sim_time(0.0);
        }
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
                ExecRequest::Trim(mode) => self.do_trim(mode)?,
                ExecRequest::InitRunning(n) => self.init_running(n)?,
                ExecRequest::Linearize => self.do_linearization()?,
            }
        }
        Ok(())
    }
}

impl Fdm {
    /// `FGInitialCondition::Load(file)` for `dir/name` (`.xml` added when
    /// it has no extension), applied to [`Models::ic`]. False, with the
    /// problem logged, where JSBSim's `Load` returns false.
    pub fn load_initial_conditions(
        &mut self,
        resolver: &dyn crate::Resolver,
        dir: &str,
        name: &str,
    ) -> Result<bool, Error> {
        let loader = crate::xml::DocumentLoader::new(resolver, dir);
        let document = loader.load_document(&format!("{dir}/{name}"))?;
        self.load_initial_conditions_document(&document)
    }

    /// `FGInitialCondition::Load` on a parsed `<initialize>` document.
    pub fn load_initial_conditions_document(
        &mut self,
        document: &crate::xml::Element,
    ) -> Result<bool, Error> {
        let m = &mut self.models;
        let outcome = m.ic.load(document, &m.atmosphere, &mut m.inertial)?;
        if outcome.planet_rotation_changed {
            m.propagate.input.omega_planet = m.inertial.omega_planet;
            m.accelerations.input.omega_planet = m.inertial.omega_planet;
        }
        Ok(outcome.result)
    }
}
