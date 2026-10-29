// SPDX-License-Identifier: LGPL-2.1-or-later
// NRLMSISE-00 by Mike Picone, Alan Hedin and Doug Drob (NRL); C release
// Copyright (C) Dominik Brodowski.
// Derived from JSBSim src/models/atmosphere/MSIS/nrlmsise-00.c (gts7).
//
//! `gts7`: the thermospheric part of the model, valid from 72.5 km up.
//! Each species' density diffuses from its lower-boundary value and, below
//! its turbopause cut-off altitude, blends into the fully mixed density
//! with turbopause and chemistry corrections.

use super::context::Context;
use super::globe::{glob7s, globe7};
use super::lower_coefficients::{PDL, PDM, PMA, PTL, PTM};
use super::profile::{Thermo, ccor, ccor2, densu, dnet, scalh};
use super::upper_coefficients::{PD, PS, PT};
use super::{Input, Output};

/// Degrees to radians, as the model rounds it.
const DGTR: f64 = 1.74533E-2;
/// Day of year to radians.
const DR: f64 = 1.72142E-2;
/// Thermal diffusion coefficients, by output density index.
const ALPHA: [f64; 9] = [-0.38, 0.0, 0.0, 0.0, 0.17, 0.0, -0.38, 0.0, 0.0];
/// Altitudes below which each species blends into the mixed density, km.
const ALTL: [f64; 8] = [200.0, 300.0, 160.0, 250.0, 240.0, 450.0, 320.0, 450.0];

/// What every species shares: the profile, the evaluation altitude `z`,
/// the mixed molecular weight `xmm`, the N2 turbopause scale `zhm28`, the
/// N2 mixed density at the turbopause `b28`, and whether switch 15
/// (departures from diffusive equilibrium) is on.
struct Species {
    th: Thermo,
    z: f64,
    xmm: f64,
    zhm28: f64,
    b28: f64,
    sw15: bool,
}

impl Species {
    /// Diffusive density at `z` of weight `xm` (alpha `alpha`) from `db` at
    /// the lower boundary.
    fn diffusive(&self, db: f64, xm: f64, alpha: f64, tz: &mut f64) -> f64 {
        densu(&self.th, self.z, db, xm, alpha, tz)
    }

    /// The net of `diffusive` and the density mixed down from turbopause
    /// `zh`, and the mixed density at `zh`.
    fn mixed(
        &self,
        diffusive: f64,
        db: f64,
        xm: f64,
        alpha: f64,
        zh: f64,
        tz: &mut f64,
    ) -> (f64, f64) {
        let b = densu(&self.th, zh, db, xm - self.xmm, alpha - 1.0, tz);
        let dm = densu(&self.th, self.z, b, self.xmm, 0., tz);
        (dnet(diffusive, dm, self.zhm28, self.xmm, xm), b)
    }
}

/// `gts7` at `input.alt` (from 72.5 km). Leaves the lower thermosphere
/// nodes and `dm28` in `ctx` for `gtd7`.
pub(super) fn gts7(input: &Input, ctx: &mut Context) -> Output {
    let sw = ctx.sw;
    let zn1 = ctx.n1.z;
    let mut output = Output {
        d: [0.0; 9],
        t: [0.0; 2],
    };

    // Tinf variations are not important below za or zn1[0].
    let tinf = if input.alt > zn1[0] {
        PTM[0] * PT[0] * (1.0 + sw[16] * globe7(&PT, input, ctx))
    } else {
        PTM[0] * PT[0]
    };
    output.t[0] = tinf;
    // Gradient variations are not important below zn1[4].
    let g0 = if input.alt > zn1[4] {
        PTM[3] * PS[0] * (1.0 + sw[19] * globe7(&PS, input, ctx))
    } else {
        PTM[3] * PS[0]
    };
    let tlb = PTM[1] * (1.0 + sw[17] * globe7(&PD[3], input, ctx)) * PD[3][0];
    let s = g0 / (tinf - tlb);
    lower_thermosphere_nodes(input, ctx);

    // N2 variation factor at zlb.
    let g28 = sw[21] * globe7(&PD[2], input, ctx);
    // Variation of turbopause height.
    let zhf = PDL[1][24]
        * (1.0
            + sw[5]
                * PDL[0][24]
                * libm::sin(DGTR * input.g_lat)
                * libm::cos(DR * (f64::from(input.doy) - PT[13])));
    output.t[0] = tinf;
    let xmm = PDM[2][4];
    let z = input.alt;
    let th = Thermo {
        tinf,
        tlb,
        zlb: PTM[5],
        s2: s,
        nodes: ctx.n1,
        grav: ctx.grav,
    };
    let mut sp = Species {
        th,
        z,
        xmm,
        zhm28: 0.0,
        b28: 0.0,
        sw15: sw[15] != 0.0,
    };
    nitrogen(&mut sp, g28, zhf, ctx, &mut output);
    helium(&sp, input, ctx, &mut output);
    oxygen(&sp, input, ctx, &mut output);
    dioxygen(&sp, input, ctx, &mut output);
    argon(&sp, input, ctx, &mut output);
    hydrogen(&sp, input, ctx, &mut output);
    atomic_nitrogen(&sp, input, ctx, &mut output);
    anomalous_oxygen(&sp, input, ctx, &mut output);

    let d = &mut output.d;
    // Total mass density.
    d[5] = 1.66E-24
        * (4.0 * d[0] + 16.0 * d[1] + 28.0 * d[2] + 32.0 * d[3] + 40.0 * d[4] + d[6] + 14.0 * d[7]);

    // Temperature.
    let z = libm::sqrt(input.alt * input.alt);
    densu(&sp.th, z, 1.0, 0.0, 0.0, &mut output.t[1]);
    if sw[0] != 0.0 {
        for di in &mut output.d {
            *di *= 1.0E6;
        }
        output.d[5] /= 1000.0;
    }
    output
}

/// Lower thermosphere node temperatures and end gradient; their
/// variations matter for density only below 300 km.
fn lower_thermosphere_nodes(input: &Input, ctx: &mut Context) {
    let sw = ctx.sw;
    if input.alt < 300.0 {
        ctx.n1.t[1] = PTM[6] * PTL[0][0] / (1.0 - sw[18] * glob7s(&PTL[0], input, ctx));
        ctx.n1.t[2] = PTM[2] * PTL[1][0] / (1.0 - sw[18] * glob7s(&PTL[1], input, ctx));
        ctx.n1.t[3] = PTM[7] * PTL[2][0] / (1.0 - sw[18] * glob7s(&PTL[2], input, ctx));
        ctx.n1.t[4] = PTM[4] * PTL[3][0] / (1.0 - sw[18] * sw[20] * glob7s(&PTL[3], input, ctx));
        ctx.n1.tg[1] = PTM[8]
            * PMA[8][0]
            * (1.0 + sw[18] * sw[20] * glob7s(&PMA[8], input, ctx))
            * ctx.n1.t[4]
            * ctx.n1.t[4]
            / (libm::pow(PTM[4] * PTL[3][0], 2.0));
    } else {
        ctx.n1.t[1] = PTM[6] * PTL[0][0];
        ctx.n1.t[2] = PTM[2] * PTL[1][0];
        ctx.n1.t[3] = PTM[7] * PTL[2][0];
        ctx.n1.t[4] = PTM[4] * PTL[3][0];
        ctx.n1.tg[1] =
            PTM[8] * PMA[8][0] * ctx.n1.t[4] * ctx.n1.t[4] / (libm::pow(PTM[4] * PTL[3][0], 2.0));
    }
}

/// N2 density, and the N2 turbopause values the other species reuse.
fn nitrogen(sp: &mut Species, g28: f64, zhf: f64, ctx: &mut Context, output: &mut Output) {
    let mut tz = 0.0;
    // Diffusive density at zlb, then at altitude.
    let db28 = PDM[2][0] * libm::exp(g28) * PD[2][0];
    output.d[2] = sp.diffusive(db28, 28.0, ALPHA[2], &mut output.t[1]);
    // Turbopause.
    let zh28 = PDM[2][2] * zhf;
    sp.zhm28 = PDM[2][3] * PDL[1][5];
    let xmd = 28.0 - sp.xmm;
    // Mixed density at zlb.
    sp.b28 = densu(&sp.th, zh28, db28, xmd, ALPHA[2] - 1.0, &mut tz);
    if sp.sw15 && (sp.z <= ALTL[2]) {
        // Mixed density, then net density, at altitude.
        ctx.dm28 = densu(&sp.th, sp.z, sp.b28, sp.xmm, ALPHA[2], &mut tz);
        output.d[2] = dnet(output.d[2], ctx.dm28, sp.zhm28, sp.xmm, 28.0);
    }
}

/// He density.
fn helium(sp: &Species, input: &Input, ctx: &mut Context, output: &mut Output) {
    let g4 = ctx.sw[21] * globe7(&PD[0], input, ctx);
    let db04 = PDM[0][0] * libm::exp(g4) * PD[0][0];
    output.d[0] = sp.diffusive(db04, 4., ALPHA[0], &mut output.t[1]);
    if sp.sw15 && (sp.z < ALTL[0]) {
        let (net, b04) = sp.mixed(output.d[0], db04, 4., ALPHA[0], PDM[0][2], &mut output.t[1]);
        output.d[0] = net;
        // Correction to the specified mixing ratio at ground.
        let rl = libm::log(sp.b28 * PDM[0][1] / b04);
        let zc04 = PDM[0][4] * PDL[1][0];
        let hc04 = PDM[0][5] * PDL[1][1];
        output.d[0] *= ccor(sp.z, rl, hc04, zc04);
    }
}

/// O density.
fn oxygen(sp: &Species, input: &Input, ctx: &mut Context, output: &mut Output) {
    let g16 = ctx.sw[21] * globe7(&PD[1], input, ctx);
    let db16 = PDM[1][0] * libm::exp(g16) * PD[1][0];
    output.d[1] = sp.diffusive(db16, 16., ALPHA[1], &mut output.t[1]);
    if sp.sw15 && (sp.z <= ALTL[1]) {
        let (net, _) = sp.mixed(
            output.d[1],
            db16,
            16.0,
            ALPHA[1],
            PDM[1][2],
            &mut output.t[1],
        );
        output.d[1] = net;
        let rl = PDM[1][1] * PDL[1][16] * (1.0 + ctx.sw[1] * PDL[0][23] * (input.f107a - 150.0));
        let hc16 = PDM[1][5] * PDL[1][3];
        let zc16 = PDM[1][4] * PDL[1][2];
        let hc216 = PDM[1][5] * PDL[1][4];
        output.d[1] *= ccor2(sp.z, rl, hc16, zc16, hc216);
        // Chemistry correction.
        let hcc16 = PDM[1][7] * PDL[1][13];
        let zcc16 = PDM[1][6] * PDL[1][12];
        let rc16 = PDM[1][3] * PDL[1][14];
        output.d[1] *= ccor(sp.z, rc16, hcc16, zcc16);
    }
}

/// O2 density.
fn dioxygen(sp: &Species, input: &Input, ctx: &mut Context, output: &mut Output) {
    let g32 = ctx.sw[21] * globe7(&PD[4], input, ctx);
    let db32 = PDM[3][0] * libm::exp(g32) * PD[4][0];
    output.d[3] = sp.diffusive(db32, 32., ALPHA[3], &mut output.t[1]);
    if sp.sw15 {
        if sp.z <= ALTL[3] {
            let (net, b32) = sp.mixed(
                output.d[3],
                db32,
                32.,
                ALPHA[3],
                PDM[3][2],
                &mut output.t[1],
            );
            output.d[3] = net;
            // Correction to the specified mixing ratio at ground.
            let rl = libm::log(sp.b28 * PDM[3][1] / b32);
            let hc32 = PDM[3][5] * PDL[1][7];
            let zc32 = PDM[3][4] * PDL[1][6];
            output.d[3] *= ccor(sp.z, rl, hc32, zc32);
        }
        // Correction for the general departure from diffusive equilibrium
        // above zlb.
        let hcc32 = PDM[3][7] * PDL[1][22];
        let hcc232 = PDM[3][7] * PDL[0][22];
        let zcc32 = PDM[3][6] * PDL[1][21];
        let rc32 = PDM[3][3] * PDL[1][23] * (1. + ctx.sw[1] * PDL[0][23] * (input.f107a - 150.));
        output.d[3] *= ccor2(sp.z, rc32, hcc32, zcc32, hcc232);
    }
}

/// Ar density.
fn argon(sp: &Species, input: &Input, ctx: &mut Context, output: &mut Output) {
    let g40 = ctx.sw[21] * globe7(&PD[5], input, ctx);
    let db40 = PDM[4][0] * libm::exp(g40) * PD[5][0];
    output.d[4] = sp.diffusive(db40, 40., ALPHA[4], &mut output.t[1]);
    if sp.sw15 && (sp.z <= ALTL[4]) {
        let (net, b40) = sp.mixed(
            output.d[4],
            db40,
            40.,
            ALPHA[4],
            PDM[4][2],
            &mut output.t[1],
        );
        output.d[4] = net;
        // Correction to the specified mixing ratio at ground.
        let rl = libm::log(sp.b28 * PDM[4][1] / b40);
        let hc40 = PDM[4][5] * PDL[1][9];
        let zc40 = PDM[4][4] * PDL[1][8];
        output.d[4] *= ccor(sp.z, rl, hc40, zc40);
    }
}

/// H density.
fn hydrogen(sp: &Species, input: &Input, ctx: &mut Context, output: &mut Output) {
    let g1 = ctx.sw[21] * globe7(&PD[6], input, ctx);
    let db01 = PDM[5][0] * libm::exp(g1) * PD[6][0];
    output.d[6] = sp.diffusive(db01, 1., ALPHA[6], &mut output.t[1]);
    if sp.sw15 && (sp.z <= ALTL[6]) {
        let (net, b01) = sp.mixed(output.d[6], db01, 1., ALPHA[6], PDM[5][2], &mut output.t[1]);
        output.d[6] = net;
        // Correction to the specified mixing ratio at ground.
        let rl = libm::log(sp.b28 * PDM[5][1] * libm::sqrt(PDL[1][17] * PDL[1][17]) / b01);
        let hc01 = PDM[5][5] * PDL[1][11];
        let zc01 = PDM[5][4] * PDL[1][10];
        output.d[6] *= ccor(sp.z, rl, hc01, zc01);
        // Chemistry correction.
        let hcc01 = PDM[5][7] * PDL[1][19];
        let zcc01 = PDM[5][6] * PDL[1][18];
        let rc01 = PDM[5][3] * PDL[1][20];
        output.d[6] *= ccor(sp.z, rc01, hcc01, zcc01);
    }
}

/// N density.
fn atomic_nitrogen(sp: &Species, input: &Input, ctx: &mut Context, output: &mut Output) {
    let g14 = ctx.sw[21] * globe7(&PD[7], input, ctx);
    let db14 = PDM[6][0] * libm::exp(g14) * PD[7][0];
    output.d[7] = sp.diffusive(db14, 14., ALPHA[7], &mut output.t[1]);
    if sp.sw15 && (sp.z <= ALTL[7]) {
        let (net, b14) = sp.mixed(
            output.d[7],
            db14,
            14.,
            ALPHA[7],
            PDM[6][2],
            &mut output.t[1],
        );
        output.d[7] = net;
        // Correction to the specified mixing ratio at ground.
        let rl = libm::log(sp.b28 * PDM[6][1] * libm::sqrt(PDL[0][2] * PDL[0][2]) / b14);
        let hc14 = PDM[6][5] * PDL[0][1];
        let zc14 = PDM[6][4] * PDL[0][0];
        output.d[7] *= ccor(sp.z, rl, hc14, zc14);
        // Chemistry correction.
        let hcc14 = PDM[6][7] * PDL[0][4];
        let zcc14 = PDM[6][6] * PDL[0][3];
        let rc14 = PDM[6][3] * PDL[0][5];
        output.d[7] *= ccor(sp.z, rc14, hcc14, zcc14);
    }
}

/// Anomalous O density.
fn anomalous_oxygen(sp: &Species, input: &Input, ctx: &mut Context, output: &mut Output) {
    let g16h = ctx.sw[21] * globe7(&PD[8], input, ctx);
    let db16h = PDM[7][0] * libm::exp(g16h) * PD[8][0];
    let tho = PDM[7][9] * PDL[0][6];
    let hot = Thermo {
        tinf: tho,
        tlb: tho,
        ..sp.th
    };
    let dd = densu(&hot, sp.z, db16h, 16., ALPHA[8], &mut output.t[1]);
    let zsht = PDM[7][5];
    let zmho = PDM[7][4];
    let zsho = scalh(zmho, 16.0, tho, sp.th.grav);
    output.d[8] = dd * libm::exp(-zsht / zsho * (libm::exp(-(sp.z - zmho) / zsht) - 1.));
}
