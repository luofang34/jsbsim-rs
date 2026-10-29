// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2009 James Goppert.
// Derived from JSBSim src/math/FGStateSpace.h (the Component classes
// FGLinearization uses).
//
//! The states and inputs of the state space: how each is read from the
//! simulation, written to the initial conditions or the FCS, and
//! differentiated.

use crate::fdm::Fdm;
use crate::initial_condition::Environment;
use crate::math::{E_P, E_PHI, E_PSI, E_Q, E_R, E_THT, E_X, E_Y, E_Z};

/// One state or input (`FGStateSpace::Component`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    /// True airspeed, ft/s.
    Vt,
    /// Angle of attack, rad.
    Alpha,
    /// Pitch angle, rad.
    Theta,
    /// Pitch rate, rad/s.
    Q,
    /// Propeller speed of engine `n` (`Rpm0` to `Rpm3`), rev/min.
    Rpm(usize),
    /// Sideslip, rad.
    Beta,
    /// Roll angle, rad.
    Phi,
    /// Roll rate, rad/s.
    P,
    /// Heading, rad.
    Psi,
    /// Yaw rate, rad/s.
    R,
    /// Geocentric latitude, rad.
    Latitude,
    /// Longitude, rad.
    Longitude,
    /// Altitude above sea level, ft.
    Alt,
    /// Throttle command of every engine, as engine 0's.
    ThrottleCmd,
    /// Aileron command.
    DaCmd,
    /// Elevator command.
    DeCmd,
    /// Rudder command.
    DrCmd,
}

impl Component {
    /// `getName`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Vt => "Vt",
            Self::Alpha => "Alpha",
            Self::Theta => "Theta",
            Self::Q => "Q",
            Self::Rpm(0) => "Rpm0",
            Self::Rpm(1) => "Rpm1",
            Self::Rpm(2) => "Rpm2",
            Self::Rpm(_) => "Rpm3",
            Self::Beta => "Beta",
            Self::Phi => "Phi",
            Self::P => "P",
            Self::Psi => "Psi",
            Self::R => "R",
            Self::Latitude => "Latitude",
            Self::Longitude => "Longitude",
            Self::Alt => "Alt",
            Self::ThrottleCmd => "ThtlCmd",
            Self::DaCmd => "DaCmd",
            Self::DeCmd => "DeCmd",
            Self::DrCmd => "DrCmd",
        }
    }

    /// `getUnit`.
    #[must_use]
    pub fn unit(self) -> &'static str {
        match self {
            Self::Vt => "ft/s",
            Self::Alpha | Self::Theta | Self::Beta | Self::Phi | Self::Psi => "rad",
            Self::Latitude | Self::Longitude => "rad",
            Self::Q | Self::P | Self::R => "rad/s",
            Self::Rpm(_) => "rev/min",
            Self::Alt => "ft",
            Self::ThrottleCmd | Self::DaCmd | Self::DeCmd | Self::DrCmd => "norm",
        }
    }

    /// `get`.
    #[must_use]
    pub fn get(self, fdm: &Fdm) -> f64 {
        let m = &fdm.models;
        let p = &m.propagate;
        let euler = || p.state.q_attitude_local.euler();
        match self {
            Self::Vt => m.auxiliary.vt,
            Self::Alpha => m.auxiliary.alpha,
            Self::Theta => euler()[E_THT],
            Self::Q => p.state.pqr[E_Q],
            Self::Rpm(n) => m
                .propulsion
                .engines
                .get(n)
                .map_or(f64::NAN, |e| e.thruster.rpm()),
            Self::Beta => m.auxiliary.beta,
            Self::Phi => euler()[E_PHI],
            Self::P => p.state.pqr[E_P],
            Self::Psi => euler()[E_PSI],
            Self::R => p.state.pqr[E_R],
            Self::Latitude => p.state.location.latitude(),
            Self::Longitude => p.state.location.longitude(),
            Self::Alt => p.altitude_asl(),
            Self::ThrottleCmd => m
                .fcs
                .throttles
                .throttle_cmd
                .first()
                .copied()
                .unwrap_or(f64::NAN),
            Self::DaCmd => m.fcs.da_cmd,
            Self::DeCmd => m.fcs.de_cmd,
            Self::DrCmd => m.fcs.dr_cmd,
        }
    }

    /// `set`: a state goes to the initial conditions, a command to the
    /// FCS (followed by `FCS->Run(true)`, which only steps its rate
    /// counter), a propeller speed to the propeller.
    pub fn set(self, fdm: &mut Fdm, val: f64) {
        let m = &mut fdm.models;
        let ic = &mut m.ic;
        let env = Environment {
            atmosphere: &m.atmosphere,
            inertial: &m.inertial,
        };
        match self {
            Self::Vt => ic.set_vtrue_fps(val),
            Self::Alpha => {
                // JSBSim hands the sideslip in degrees to the radian setter.
                let beta = ic.beta_deg();
                let psi = ic.euler_rad(E_PSI);
                let theta = ic.euler_rad(E_THT);
                ic.set_alpha_rad(val);
                ic.set_beta_rad(beta);
                ic.set_euler_angle_rad(E_PSI, psi);
                ic.set_euler_angle_rad(E_THT, theta);
            }
            Self::Theta => {
                let gamma = val - ic.alpha_rad();
                ic.set_flight_path_angle_rad(gamma);
            }
            Self::Q => ic.set_pqr_radps(E_Q, val),
            Self::Rpm(n) => {
                if let Some(e) = m.propulsion.engines.get_mut(n) {
                    e.thruster.set_rpm(val);
                }
            }
            Self::Beta => {
                let psi = ic.euler_rad(E_PSI);
                ic.set_beta_rad(val);
                ic.set_euler_angle_rad(E_PSI, psi);
            }
            Self::Phi => ic.set_euler_angle_rad(E_PHI, val),
            Self::P => ic.set_pqr_radps(E_P, val),
            Self::Psi => ic.set_euler_angle_rad(E_PSI, val),
            Self::R => ic.set_pqr_radps(E_R, val),
            Self::Latitude => ic.set_latitude_rad(env, val),
            Self::Longitude => ic.set_longitude_rad(env, val),
            Self::Alt => ic.set_altitude_asl_ft(env, val),
            Self::ThrottleCmd => {
                for i in 0..m.propulsion.engines.len() {
                    let n = i32::try_from(i).unwrap_or(i32::MAX);
                    m.fcs.throttles.set_throttle_cmd(n, val);
                }
                m.fcs.base.skip();
            }
            Self::DaCmd | Self::DeCmd | Self::DrCmd => {
                let cmd = match self {
                    Self::DaCmd => &mut m.fcs.da_cmd,
                    Self::DeCmd => &mut m.fcs.de_cmd,
                    _ => &mut m.fcs.dr_cmd,
                };
                *cmd = val;
                m.fcs.base.skip();
            }
        }
    }

    /// The overridden `getDeriv`: the time derivative from the models'
    /// current rates, or `None` for the components that take JSBSim's
    /// default, a frame run through the simulation.
    #[must_use]
    pub fn analytic_deriv(self, fdm: &Fdm) -> Option<f64> {
        let m = &fdm.models;
        let p = &m.propagate;
        let (aux, acc) = (&m.auxiliary, &m.accelerations);
        Some(match self {
            Self::Vt => {
                let (uvw, dot) = (p.state.uvw, acc.uvwdot);
                (uvw[E_X] * dot[E_X] + uvw[E_Y] * dot[E_Y] + uvw[E_Z] * dot[E_Z]) / aux.vt
            }
            Self::Alpha => aux.adot,
            Self::Theta => aux.euler_rates[E_THT],
            Self::Q => acc.pqrdot[E_Q],
            Self::Alt => p.hdot(),
            Self::Beta => aux.bdot,
            Self::Phi => aux.euler_rates[E_PHI],
            Self::P => acc.pqrdot[E_P],
            Self::Psi => aux.euler_rates[E_PSI],
            Self::R => acc.pqrdot[E_R],
            Self::Longitude => {
                let latitude = p.state.location.latitude();
                p.vel[E_Y] / (libm::cos(latitude) * p.radius())
            }
            Self::Latitude => p.vel[E_X] / p.radius(),
            Self::Rpm(_) | Self::ThrottleCmd | Self::DaCmd | Self::DeCmd | Self::DrCmd => {
                return None;
            }
        })
    }
}
