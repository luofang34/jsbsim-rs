// SPDX-License-Identifier: LGPL-2.1-or-later
// FGFCS: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// FGFCSComponent, FGFCSFunction: Copyright (C) 2000-2005 Jon S. Berndt.
// Derived from JSBSim src/models/FGFCS.{h,cpp} and
// src/models/flight_control/FGFCSChannel.h.
//
//! Flight control, autopilot and system channels and their components.
//! [`families`] says how component families are registered; a component
//! no family registers yet is refused with
//! [`Error::Unsupported`](crate::Error::Unsupported).

pub mod accelerometer;
mod bind;
pub mod component;
pub mod deadband;
pub mod families;
pub mod filter;
pub mod function;
pub mod gain;
pub mod gyro;
mod load;
pub mod magnetometer;
pub mod pid;
pub mod sensor;
pub mod summer;
pub mod switch;
#[cfg(test)]
mod tests;
pub mod waypoint;

pub use bind::bind;
pub use families::{Component, ComponentAt, Kind};

use super::base::ModelBase;
use crate::properties::NodeId;
/// Surface position forms (`OutputForm`): radians, degrees, normalised,
/// and the magnitude in radians.
pub const NFORMS: usize = 4;
const OF_RAD: usize = 0;
const OF_DEG: usize = 1;
const OF_NORM: usize = 2;
const OF_MAG: usize = 3;

/// A surface position in every form (`SetDaLPos` and friends).
#[derive(Debug, Clone, Copy, Default)]
pub struct Surface(pub [f64; NFORMS]);

impl Surface {
    /// `SetD*Pos(form, pos)`.
    pub fn set(&mut self, form: usize, pos: f64) {
        let p = &mut self.0;
        match form {
            OF_RAD => {
                p[OF_RAD] = pos;
                p[OF_DEG] = pos * crate::units::RADTODEG;
            }
            OF_DEG => {
                p[OF_RAD] = pos * crate::units::DEGTORAD;
                p[OF_DEG] = pos;
            }
            OF_NORM => p[OF_NORM] = pos,
            _ => {}
        }
        p[OF_MAG] = p[OF_RAD].abs();
    }
}

/// A channel of components (`FGFCSChannel`).
#[derive(Debug, Clone)]
pub struct Channel {
    /// Channel name.
    pub name: String,
    /// Runs every `exec_rate` frames.
    pub exec_rate: i32,
    /// Frames since the last run.
    pub frames_since_run: i32,
    /// Property that switches the channel on and off.
    pub on_off: Option<NodeId>,
    /// Components in execution order.
    pub components: Vec<Component>,
}

impl Channel {
    fn new(name: String, exec_rate: i32, on_off: Option<NodeId>) -> Self {
        let exec_rate = exec_rate.max(1);
        Self {
            name,
            exec_rate,
            frames_since_run: exec_rate,
            on_off,
            components: Vec::new(),
        }
    }

    /// `Execute`'s scheduling: whether the components run this frame. The
    /// caller has already checked the on/off property.
    pub fn due(&mut self, dt: f64, trim_status: bool) -> bool {
        if dt != 0.0 {
            if self.frames_since_run >= self.exec_rate {
                self.frames_since_run = 0;
            }
            self.frames_since_run = self.frames_since_run.wrapping_add(1);
        }
        trim_status || self.frames_since_run >= self.exec_rate
    }

    /// `Reset`: every component's past states, and the frame count so the
    /// components run on the next frame.
    pub fn reset(&mut self) {
        for component in &mut self.components {
            component.reset();
        }
        self.frames_since_run = self.exec_rate;
    }
}

/// `FGFCS`.
#[derive(Debug, Clone, Default)]
pub struct Fcs {
    /// Scheduling.
    pub base: ModelBase,
    /// Aileron, elevator, rudder, flap, speedbrake, spoiler commands.
    pub da_cmd: f64,
    /// See `da_cmd`.
    pub de_cmd: f64,
    /// See `da_cmd`.
    pub dr_cmd: f64,
    /// See `da_cmd`.
    pub df_cmd: f64,
    /// See `da_cmd`.
    pub dsb_cmd: f64,
    /// See `da_cmd`.
    pub dsp_cmd: f64,
    /// Trim commands.
    pub ptrim_cmd: f64,
    /// See `ptrim_cmd`.
    pub ytrim_cmd: f64,
    /// See `ptrim_cmd`.
    pub rtrim_cmd: f64,
    /// Surfaces: left and right aileron, elevator, rudder, flap,
    /// speedbrake, spoiler.
    pub dal: Surface,
    /// See `dal`.
    pub dar: Surface,
    /// See `dal`.
    pub de: Surface,
    /// See `dal`.
    pub dr: Surface,
    /// See `dal`.
    pub df: Surface,
    /// See `dal`.
    pub dsb: Surface,
    /// See `dal`.
    pub dsp: Surface,
    /// Gear command and position (1 is down).
    pub gear_cmd: f64,
    /// See `gear_cmd`.
    pub gear_pos: f64,
    /// Brake positions by brake group (none, left, right, centre, nose,
    /// tail).
    pub brake_pos: [f64; 6],
    /// Tail hook position.
    pub tailhook_pos: f64,
    /// Wing fold position.
    pub wing_fold_pos: f64,
    /// Rate of the channel being executed (`ChannelRate`).
    pub channel_rate: i32,
    /// Channels in load order.
    pub channels: Vec<Channel>,
}

impl Fcs {
    /// `FGFCS()`: gear down.
    #[must_use]
    pub fn new() -> Self {
        Self {
            gear_cmd: 1.0,
            gear_pos: 1.0,
            channel_rate: 1,
            ..Self::default()
        }
    }

    /// `InitModel`.
    pub fn init_model(&mut self) {
        self.base.init();
        self.da_cmd = 0.0;
        self.de_cmd = 0.0;
        self.dr_cmd = 0.0;
        self.df_cmd = 0.0;
        self.dsb_cmd = 0.0;
        self.dsp_cmd = 0.0;
        self.ptrim_cmd = 0.0;
        self.ytrim_cmd = 0.0;
        self.rtrim_cmd = 0.0;
        self.tailhook_pos = 0.0;
        self.wing_fold_pos = 0.0;
        for s in [
            &mut self.de,
            &mut self.dal,
            &mut self.dar,
            &mut self.dr,
            &mut self.df,
            &mut self.dsb,
            &mut self.dsp,
        ] {
            *s = Surface::default();
        }
        for channel in &mut self.channels {
            channel.reset();
        }
    }

    /// `GetDt`: the executive step `exec_dt` times this model's rate.
    #[must_use]
    pub fn dt(&self, exec_dt: f64) -> f64 {
        exec_dt * f64::from(self.base.rate)
    }

    /// `GetChannelDeltaT`.
    #[must_use]
    pub fn channel_delta_t(&self, exec_dt: f64) -> f64 {
        self.dt(exec_dt) * f64::from(self.channel_rate)
    }
}
