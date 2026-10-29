// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2011  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/atmosphere/FGWinds.cpp (the POE table
// of the constructor, and the ttMilspec and ttTustin cases of
// Turbulence).
//
//! The Dryden turbulence of MIL-F-8785C, discretised as MIL-STD-1797A
//! does (`ttMilspec`) or by Tustin's method (`ttTustin`), after Yeager,
//! "Implementation and Testing of Turbulence Models for the F18-HARV
//! Simulation" (NASA CR-1998-206937). Equation numbers are Yeager's.
//!
//! Every frame draws four normal deviates, u, v, w and p in that order;
//! the expressions keep JSBSim's operation order, so that the filters
//! round as C++ rounds them.

use std::f64::consts::PI;

use super::super::Winds;
use crate::functions::Table;
use crate::math::{ColumnVector3, E_DOWN, E_EAST, E_NORTH, E_P, E_Q, E_R};
use crate::random::RandomNumberGenerator;

/// The filters' past outputs (`xi_*`) and inputs (`nu_*`), one and two
/// frames back; `InitModel` clears them.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DrydenState {
    /// `xi_u_km1`.
    pub xi_u_km1: f64,
    /// `nu_u_km1`.
    pub nu_u_km1: f64,
    /// `xi_v_km1`.
    pub xi_v_km1: f64,
    /// `xi_v_km2`.
    pub xi_v_km2: f64,
    /// `nu_v_km1`.
    pub nu_v_km1: f64,
    /// `nu_v_km2`.
    pub nu_v_km2: f64,
    /// `xi_w_km1`.
    pub xi_w_km1: f64,
    /// `xi_w_km2`.
    pub xi_w_km2: f64,
    /// `nu_w_km1`.
    pub nu_w_km1: f64,
    /// `nu_w_km2`.
    pub nu_w_km2: f64,
    /// `xi_p_km1`.
    pub xi_p_km1: f64,
    /// `nu_p_km1`.
    pub nu_p_km1: f64,
    /// `xi_q_km1`.
    pub xi_q_km1: f64,
    /// `xi_r_km1`.
    pub xi_r_km1: f64,
}

/// `POE_Table`: Figure 7 of MIL-F-8785C (p. 49), the turbulence intensity
/// sigma, ft/s, by probability of exceedance curve (rows, the severity)
/// and altitude, ft (columns).
#[derive(Debug, Clone)]
pub struct PoeTable(Table);

const POE_ALTITUDES: [f64; 12] = [
    500.0, 1750.0, 3750.0, 7500.0, 15000.0, 25000.0, 35000.0, 45000.0, 55000.0, 65000.0, 75000.0,
    80000.0,
];

const POE_ROWS: [(f64, [f64; 12]); 7] = [
    (
        1.0,
        [3.2, 2.2, 1.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ),
    (
        2.0,
        [4.2, 3.6, 3.3, 1.6, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ),
    (
        3.0,
        [6.6, 6.9, 7.4, 6.7, 4.6, 2.7, 0.4, 0.0, 0.0, 0.0, 0.0, 0.0],
    ),
    (
        4.0,
        [8.6, 9.6, 10.6, 10.1, 8.0, 6.6, 5.0, 4.2, 2.7, 0.0, 0.0, 0.0],
    ),
    (
        5.0,
        [
            11.8, 13.0, 16.0, 15.1, 11.6, 9.7, 8.1, 8.2, 7.9, 4.9, 3.2, 2.1,
        ],
    ),
    (
        6.0,
        [
            15.6, 17.6, 23.0, 23.6, 22.1, 20.0, 16.0, 15.1, 12.1, 7.9, 6.2, 5.1,
        ],
    ),
    (
        7.0,
        [
            18.7, 21.5, 28.4, 30.2, 30.7, 31.0, 25.2, 23.1, 17.5, 10.7, 8.4, 7.2,
        ],
    ),
];

impl Default for PoeTable {
    fn default() -> Self {
        let refs: Vec<(f64, &[f64])> = POE_ROWS.iter().map(|(k, v)| (*k, v.as_slice())).collect();
        Self(Table::two_d(&POE_ALTITUDES, &refs))
    }
}

impl PoeTable {
    /// `POE_Table->GetValue(severity, h)`.
    #[must_use]
    pub fn sigma(&self, severity: i32, h: f64) -> f64 {
        self.0.value_2d(f64::from(severity), h)
    }
}

/// `sqr`.
fn sqr(x: f64) -> f64 {
    x * x
}

/// The scale lengths `L_u`, `L_w`, ft, and intensities `sig_u`, `sig_w`,
/// ft/s, at height `h`, ft (MIL-F-8785C Figs. 10 and 11, linearly
/// blended between 1000 and 2000 ft into the high altitude model).
fn scales(h: f64, windspeed_20ft: f64, sigma: impl Fn() -> f64) -> [f64; 4] {
    if h <= 1000.0 {
        let l_u = h / crate::elementary::pow(0.177 + 0.000823 * h, 1.2);
        let sig_w = 0.1 * windspeed_20ft;
        let sig_u = sig_w / crate::elementary::pow(0.177 + 0.000823 * h, 0.4);
        [l_u, h, sig_u, sig_w]
    } else if h <= 2000.0 {
        let l = 1000.0 + (h - 1000.0) / 1000.0 * 750.0;
        let sig = 0.1 * windspeed_20ft + (h - 1000.0) / 1000.0 * (sigma() - 0.1 * windspeed_20ft);
        [l, l, sig, sig]
    } else {
        let sig = sigma();
        [1750.0, 1750.0, sig, sig]
    }
}

/// What both discretisations need for one frame.
struct Frame {
    v: f64,
    b_w: f64,
    t_v: f64,
    l_u: f64,
    l_w: f64,
    sig_u: f64,
    sig_w: f64,
    sig_p: f64,
    tau_u: f64,
    tau_w: f64,
    tau_p: f64,
    tau_q: f64,
    tau_r: f64,
    /// `nu_u`, `nu_v`, `nu_w`, `nu_p`.
    nu: [f64; 4],
}

/// `xi_u` .. `xi_r`.
type Xi = [f64; 6];

/// Tustin's discretisation, eqs. (18) to (26).
fn tustin(f: &Frame, s: &DrydenState) -> Xi {
    let (v, t_v, b_w) = (f.v, f.t_v, f.b_w);
    let [nu_u, nu_v, nu_w, nu_p] = f.nu;
    let sqrt3 = 3f64.sqrt();
    let omega_w = v / f.l_w;
    let omega_v = v / f.l_u;
    let c_bl = 1.0 / f.tau_u / libm::tan(t_v / 2.0 / f.tau_u);
    let c_blp = 1.0 / f.tau_p / libm::tan(t_v / 2.0 / f.tau_p);
    let c_blq = 1.0 / f.tau_q / libm::tan(t_v / 2.0 / f.tau_q);
    let c_blr = 1.0 / f.tau_r / libm::tan(t_v / 2.0 / f.tau_r);
    let xi_u = -(1.0 - c_bl * f.tau_u) / (1.0 + c_bl * f.tau_u) * s.xi_u_km1
        + f.sig_u * (2.0 * f.tau_u / t_v).sqrt() / (1.0 + c_bl * f.tau_u) * (nu_u + s.nu_u_km1);
    let second_order = |omega: f64, sig: f64, xi1: f64, xi2: f64, nu: f64, nu1: f64, nu2: f64| {
        -2.0 * (sqr(omega) - sqr(c_bl)) / sqr(omega + c_bl) * xi1
            - sqr(omega - c_bl) / sqr(omega + c_bl) * xi2
            + sig * (3.0 * omega / t_v).sqrt() / sqr(omega + c_bl)
                * ((c_bl + omega / sqrt3) * nu
                    + 2.0 / sqrt3 * omega * nu1
                    + (omega / sqrt3 - c_bl) * nu2)
    };
    let xi_v = second_order(
        omega_v, f.sig_u, s.xi_v_km1, s.xi_v_km2, nu_v, s.nu_v_km1, s.nu_v_km2,
    );
    let xi_w = second_order(
        omega_w, f.sig_w, s.xi_w_km1, s.xi_w_km2, nu_w, s.nu_w_km1, s.nu_w_km2,
    );
    let xi_p = -(1.0 - c_blp * f.tau_p) / (1.0 + c_blp * f.tau_p) * s.xi_p_km1
        + f.sig_p * (2.0 * f.tau_p / t_v).sqrt() / (1.0 + c_blp * f.tau_p) * (nu_p + s.nu_p_km1);
    let kq = 4.0 * b_w * c_blq / PI / v;
    let xi_q = -(1.0 - kq) / (1.0 + kq) * s.xi_q_km1 + c_blq / v / (1.0 + kq) * (xi_w - s.xi_w_km1);
    let kr = 3.0 * b_w * c_blr / PI / v;
    let xi_r = -(1.0 - kr) / (1.0 + kr) * s.xi_r_km1 + c_blr / v / (1.0 + kr) * (xi_v - s.xi_v_km1);
    [xi_u, xi_v, xi_w, xi_p, xi_q, xi_r]
}

/// MIL-STD-1797A's discretisation, eqs. (30) to (35).
fn milspec(f: &Frame, s: &DrydenState) -> Xi {
    let (t_v, b_w) = (f.t_v, f.b_w);
    let [nu_u, nu_v, nu_w, nu_p] = f.nu;
    let xi_u = (1.0 - t_v / f.tau_u) * s.xi_u_km1 + f.sig_u * (2.0 * t_v / f.tau_u).sqrt() * nu_u;
    let xi_v =
        (1.0 - 2.0 * t_v / f.tau_u) * s.xi_v_km1 + f.sig_u * (4.0 * t_v / f.tau_u).sqrt() * nu_v;
    let xi_w =
        (1.0 - 2.0 * t_v / f.tau_w) * s.xi_w_km1 + f.sig_w * (4.0 * t_v / f.tau_w).sqrt() * nu_w;
    let xi_p = (1.0 - t_v / f.tau_p) * s.xi_p_km1 + f.sig_p * (2.0 * t_v / f.tau_p).sqrt() * nu_p;
    let xi_q = (1.0 - t_v / f.tau_q) * s.xi_q_km1 + PI / 4.0 / b_w * (xi_w - s.xi_w_km1);
    let xi_r = (1.0 - t_v / f.tau_r) * s.xi_r_km1 + PI / 3.0 / b_w * (xi_v - s.xi_v_km1);
    [xi_u, xi_v, xi_w, xi_p, xi_q, xi_r]
}

impl Winds {
    /// The `ttMilspec` (or, with `tustin`, `ttTustin`) case; false when
    /// a zero severity or airspeed returns early.
    pub(super) fn dryden_step(&mut self, rng: &RandomNumberGenerator, tustin_form: bool) -> bool {
        // The time constants use the airspeed relative to the air mass,
        // which excludes the turbulence: fed back, the Tustin filters drift.
        let airmass = self.input.tl2b * (self.wind_ned + self.gust_ned + self.cosine_gust_ned);
        let v = (self.input.uvw - airmass).magnitude();
        let t = self.turbulence;
        if t.severity == 0 || v == 0.0 {
            self.turbulence_ned = ColumnVector3::zero();
            self.turb_pqr = ColumnVector3::zero();
            return false;
        }
        let b_w = if self.input.wingspan == 0.0 {
            30.0
        } else {
            self.input.wingspan
        };
        // Clipped at 10 ft; a NaN altitude stays NaN, as in C++.
        let mut h = self.input.altitude_asl;
        if h <= 10.0 {
            h = 10.0;
        }
        let poe = &self.poe_table;
        let [l_u, l_w, sig_u, sig_w] = scales(h, t.windspeed_at_20ft, || poe.sigma(t.severity, h));
        let l_p = (l_w * b_w).sqrt() / 2.6;
        let frame = Frame {
            v,
            b_w,
            t_v: self.input.total_delta_t,
            l_u,
            l_w,
            sig_u,
            sig_w,
            sig_p: 1.9 / (l_w * b_w).sqrt() * sig_w,
            tau_u: l_u / v,
            tau_w: l_w / v,
            tau_p: l_p / v,
            tau_q: 4.0 * b_w / PI / v,
            tau_r: 3.0 * b_w / PI / v,
            nu: [rng.normal(), rng.normal(), rng.normal(), rng.normal()],
        };
        let s = self.dryden;
        let xi = if tustin_form {
            tustin(&frame, &s)
        } else {
            milspec(&frame, &s)
        };
        self.apply_dryden(xi, &frame, s);
        true
    }

    /// Rotates the filter outputs by the wind azimuth into the velocities
    /// and rates, and hands them on to the next frame.
    fn apply_dryden(&mut self, xi: Xi, frame: &Frame, s: DrydenState) {
        let [xi_u, xi_v, xi_w, xi_p, xi_q, xi_r] = xi;
        let (sinpsi, cospsi) = libm::sincos(self.psiw);
        self.turbulence_ned[E_NORTH] = cospsi * xi_u + sinpsi * xi_v;
        self.turbulence_ned[E_EAST] = -sinpsi * xi_u + cospsi * xi_v;
        self.turbulence_ned[E_DOWN] = xi_w;
        let mut pqr = ColumnVector3::zero();
        pqr[E_P] = cospsi * xi_p + sinpsi * xi_q;
        pqr[E_Q] = -sinpsi * xi_p + cospsi * xi_q;
        pqr[E_R] = xi_r;
        // The rates are in the body frame, not NED.
        self.turb_pqr = self.input.tl2b * pqr;
        let [nu_u, nu_v, nu_w, nu_p] = frame.nu;
        self.dryden = DrydenState {
            xi_u_km1: xi_u,
            nu_u_km1: nu_u,
            xi_v_km1: xi_v,
            xi_v_km2: s.xi_v_km1,
            nu_v_km1: nu_v,
            nu_v_km2: s.nu_v_km1,
            xi_w_km1: xi_w,
            xi_w_km2: s.xi_w_km1,
            nu_w_km1: nu_w,
            nu_w_km2: s.nu_w_km1,
            xi_p_km1: xi_p,
            nu_p_km1: nu_p,
            xi_q_km1: xi_q,
            xi_r_km1: xi_r,
        };
    }
}

crate::snapshot::visit_fields! {
    DrydenState {
        xi_u_km1, nu_u_km1, xi_v_km1, xi_v_km2, nu_v_km1, nu_v_km2, xi_w_km1, xi_w_km2, nu_w_km1,
        nu_w_km2, xi_p_km1, nu_p_km1, xi_q_km1, xi_r_km1,
    }
}
