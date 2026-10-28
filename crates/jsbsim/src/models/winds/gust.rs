// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/atmosphere/FGWinds.{h,cpp}
// (OneMinusCosineProfile, OneMinusCosineGust, CosineGustProfile, CosineGust).
//
//! The one-minus-cosine gust: a gust that builds up over a startup
//! duration, holds for a steady duration and fades over an end duration,
//! blowing along a direction fixed in the local frame when it starts.

use std::f64::consts::PI;

use super::Winds;
use crate::math::ColumnVector3;

/// `eGustFrame`: the frame the gust direction is given in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GustFrame {
    /// `gfNone`.
    None,
    /// `gfBody`.
    Body,
    /// `gfWind`.
    Wind,
    /// `gfLocal`: north, east, down.
    Local,
}

impl GustFrame {
    /// The frame of an `atmosphere/cosine-gust/frame` value; `None` for
    /// values outside the enum, which JSBSim's switch also ignores.
    #[must_use]
    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::None),
            1 => Some(Self::Body),
            2 => Some(Self::Wind),
            3 => Some(Self::Local),
            _ => None,
        }
    }

    /// The enum value.
    #[must_use]
    pub fn code(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Body => 1,
            Self::Wind => 2,
            Self::Local => 3,
        }
    }
}

/// `OneMinusCosineProfile`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GustProfile {
    /// `Running`: set by `atmosphere/cosine-gust/start`.
    pub running: bool,
    /// `elapsedTime` of the ongoing gust, s.
    pub elapsed_time: f64,
    /// `startupDuration`, s.
    pub startup_duration: f64,
    /// `steadyDuration`, s.
    pub steady_duration: f64,
    /// `endDuration`, s.
    pub end_duration: f64,
}

impl Default for GustProfile {
    fn default() -> Self {
        Self {
            running: false,
            elapsed_time: 0.0,
            startup_duration: 2.0,
            steady_duration: 4.0,
            end_duration: 2.0,
        }
    }
}

impl GustProfile {
    /// `CosineGustProfile`: the gust's fraction of its magnitude at
    /// `elapsed_time`. A zero startup duration gives NaN at time zero, as
    /// in JSBSim.
    #[must_use]
    pub fn factor(&self, elapsed_time: f64) -> f64 {
        let (start, steady, end) = (
            self.startup_duration,
            self.steady_duration,
            self.end_duration,
        );
        if elapsed_time >= 0.0 && elapsed_time <= start {
            (1.0 - libm::cos(PI * elapsed_time / start)) / 2.0
        } else if elapsed_time > start && elapsed_time <= start + steady {
            1.0
        } else if elapsed_time > start + steady && elapsed_time <= start + steady + end {
            (1.0 - libm::cos(PI * (1.0 - (elapsed_time - (start + steady)) / end))) / 2.0
        } else {
            0.0
        }
    }
}

/// `OneMinusCosineGust`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CosineGust {
    /// `vWind`: the direction as given, normalized in place every frame the
    /// gust runs.
    pub direction: ColumnVector3,
    /// `vWindTransformed`: the direction in the local frame, fixed when the
    /// gust starts and cleared when it ends.
    pub direction_local: ColumnVector3,
    /// `magnitude`, ft/s.
    pub magnitude: f64,
    /// `gustFrame`, stored as written (any `int`).
    pub frame: i32,
    /// The time history.
    pub profile: GustProfile,
}

impl Default for CosineGust {
    fn default() -> Self {
        Self {
            direction: ColumnVector3::zero(),
            direction_local: ColumnVector3::zero(),
            magnitude: 1.0,
            frame: GustFrame::Local.code(),
            profile: GustProfile::default(),
        }
    }
}

impl Winds {
    /// `FGWinds::CosineGust`: one step of the running gust.
    pub(super) fn cosine_gust_step(&mut self) {
        let g = &mut self.cosine_gust;
        let factor = g.profile.factor(g.profile.elapsed_time);
        g.direction.normalize();
        if g.direction_local.magnitude() == 0.0 {
            let lb = self.input.tl2b.inverse();
            match GustFrame::from_code(g.frame) {
                Some(GustFrame::Body) => g.direction_local = lb * g.direction,
                Some(GustFrame::Wind) => {
                    g.direction_local = lb * self.input.tw2b * g.direction;
                }
                Some(GustFrame::Local) => g.direction_local = g.direction,
                Some(GustFrame::None) | None => {}
            }
        }
        self.cosine_gust_ned = factor * g.direction_local * g.magnitude;
        let p = &mut g.profile;
        p.elapsed_time += self.input.total_delta_t;
        if p.elapsed_time > p.startup_duration + p.steady_duration + p.end_duration {
            p.running = false;
            p.elapsed_time = 0.0;
            g.direction_local = ColumnVector3::zero();
            self.cosine_gust_ned = ColumnVector3::zero();
        }
    }
}
