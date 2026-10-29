// SPDX-License-Identifier: LGPL-2.1-or-later
// NRLMSISE-00 by Mike Picone, Alan Hedin and Doug Drob (NRL); C release
// Copyright (C) Dominik Brodowski.
// Derived from JSBSim src/models/atmosphere/MSIS/nrlmsise-00.c (its
// file-scope variables).
//
//! The intermediate results the C release shares between its routines
//! through file-scope variables (`PARMB`, `DMIX`, `MESO7`, `LPOLY`), held
//! for the length of one `gtd7` call.

use super::lower_coefficients::PDL;
use super::{ZN2, ZN3};

/// `PARMB`: surface gravity (cm/s²) and effective Earth radius (km).
#[derive(Debug, Clone, Copy)]
pub(super) struct Gravity {
    /// `gsurf`.
    pub gsurf: f64,
    /// `re`.
    pub re: f64,
}

/// One temperature profile's spline nodes: altitudes, temperatures, and the
/// temperature gradients at the first and last node (`MESO7`).
#[derive(Debug, Clone, Copy)]
pub(super) struct Nodes<const N: usize> {
    /// `zn`, km.
    pub z: [f64; N],
    /// `meso_tn`, K.
    pub t: [f64; N],
    /// `meso_tgn`, K/km.
    pub tg: [f64; 2],
}

impl<const N: usize> Nodes<N> {
    /// Nodes at `z`, every temperature zero as the C statics start.
    const fn at(z: [f64; N]) -> Self {
        Self {
            z,
            t: [0.0; N],
            tg: [0.0; 2],
        }
    }
}

/// Sines and cosines of the diurnal, semidiurnal and terdiurnal local
/// time harmonics (`stloc`, `ctloc`, `s2tloc`, `c2tloc`, `s3tloc`,
/// `c3tloc`).
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct LocalTime {
    /// `stloc`.
    pub s1: f64,
    /// `ctloc`.
    pub c1: f64,
    /// `s2tloc`.
    pub s2: f64,
    /// `c2tloc`.
    pub c2: f64,
    /// `s3tloc`.
    pub s3: f64,
    /// `c3tloc`.
    pub c3: f64,
}

/// The state one `gtd7` evaluation threads through its routines.
#[derive(Debug, Clone)]
pub(super) struct Context {
    /// `flags->sw`.
    pub sw: [f64; 24],
    /// `flags->swc`.
    pub swc: [f64; 24],
    /// `gsurf`, `re`.
    pub grav: Gravity,
    /// `dm28`: N2 mixed density at the evaluation altitude of `gts7`.
    pub dm28: f64,
    /// Lower thermosphere nodes (`zn1`, `meso_tn1`, `meso_tgn1`).
    pub n1: Nodes<5>,
    /// Lower mesosphere and upper stratosphere nodes (`zn2`, `meso_tn2`,
    /// `meso_tgn2`).
    pub n2: Nodes<4>,
    /// Lower stratosphere and troposphere nodes (`zn3`, `meso_tn3`,
    /// `meso_tgn3`).
    pub n3: Nodes<5>,
    /// `dfa`: F10.7 81-day average departure from 150.
    pub dfa: f64,
    /// `plg`: associated Legendre polynomials of the latitude.
    pub plg: [[f64; 9]; 4],
    /// Local time harmonics.
    pub tloc: LocalTime,
    /// `apdf`: daily ap magnetic activity function.
    pub apdf: f64,
    /// `apt[0]`: 3-hour ap magnetic activity function.
    pub apt0: f64,
}

impl Context {
    /// A context for switches `sw`, `swc` and gravity `grav`, everything
    /// else zero as the C statics start.
    pub fn new(sw: [f64; 24], swc: [f64; 24], grav: Gravity) -> Self {
        Self {
            sw,
            swc,
            grav,
            dm28: 0.0,
            n1: Nodes::at([PDL[1][15], 110.0, 100.0, 90.0, 72.5]),
            n2: Nodes::at(ZN2),
            n3: Nodes::at(ZN3),
            dfa: 0.0,
            plg: [[0.0; 9]; 4],
            tloc: LocalTime::default(),
            apdf: 0.0,
            apt0: 0.0,
        }
    }
}
