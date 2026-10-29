// SPDX-License-Identifier: LGPL-2.1-or-later
// NRLMSISE-00 by Mike Picone, Alan Hedin and Doug Drob (NRL); C release
// Copyright (C) Dominik Brodowski.
// Derived from JSBSim src/models/atmosphere/MSIS/nrlmsise-00.c (g0, sumex,
// sg0, globe7, glob7s).
//
//! The `G(L)` expansions: a parameter set's variation with solar flux,
//! season, latitude, local time, magnetic activity and longitude.

use tracing::warn;

use super::Input;
use super::context::{Context, LocalTime};

/// Degrees to radians, as the model rounds it.
const DGTR: f64 = 1.74533E-2;
/// Day of year to radians.
const DR: f64 = 1.72142E-2;
/// Seconds of UT to radians.
const SR: f64 = 7.2722E-5;
/// Hours of local time to radians.
const HR: f64 = 0.2618;

/// Eq. A24d, `g0`: 3-hour magnetic activity function of `a`, with `p24`
/// and `p25` standing for `p[24]` and `p[25]`.
fn g0(a: f64, p24: f64, p25: f64) -> f64 {
    a - 4.0
        + (p25 - 1.0)
            * (a - 4.0
                + (libm::exp(-libm::sqrt(p24 * p24) * (a - 4.0)) - 1.0) / libm::sqrt(p24 * p24))
}

/// Eq. A24c, `sumex`.
fn sumex(ex: f64) -> f64 {
    1.0 + (1.0 - libm::pow(ex, 19.0)) / (1.0 - ex) * libm::pow(ex, 0.5)
}

/// Eq. A24a, `sg0`: the weighted 3-hour ap history.
fn sg0(ex: f64, p24: f64, p25: f64, ap: &[f64; 7]) -> f64 {
    let g = |a: f64| g0(a, p24, p25);
    (g(ap[1])
        + (g(ap[2]) * ex
            + g(ap[3]) * ex * ex
            + g(ap[4]) * libm::pow(ex, 3.0)
            + (g(ap[5]) * libm::pow(ex, 4.0) + g(ap[6]) * libm::pow(ex, 12.0))
                * (1.0 - libm::pow(ex, 8.0))
                / (1.0 - ex)))
        / sumex(ex)
}

/// Associated Legendre polynomials of the latitude whose sine is `c` and
/// cosine `s`, into the entries of `plg` the model reads.
fn legendre(plg: &mut [[f64; 9]; 4], c: f64, s: f64) {
    let c2 = c * c;
    let c4 = c2 * c2;
    let s2 = s * s;
    plg[0][1] = c;
    plg[0][2] = 0.5 * (3.0 * c2 - 1.0);
    plg[0][3] = 0.5 * (5.0 * c * c2 - 3.0 * c);
    plg[0][4] = (35.0 * c4 - 30.0 * c2 + 3.0) / 8.0;
    plg[0][5] = (63.0 * c2 * c2 * c - 70.0 * c2 * c + 15.0 * c) / 8.0;
    plg[0][6] = (11.0 * c * plg[0][5] - 5.0 * plg[0][4]) / 6.0;
    plg[1][1] = s;
    plg[1][2] = 3.0 * c * s;
    plg[1][3] = 1.5 * (5.0 * c2 - 1.0) * s;
    plg[1][4] = 2.5 * (7.0 * c2 * c - 3.0 * c) * s;
    plg[1][5] = 1.875 * (21.0 * c4 - 14.0 * c2 + 1.0) * s;
    plg[1][6] = (11.0 * c * plg[1][5] - 6.0 * plg[1][4]) / 5.0;
    plg[2][2] = 3.0 * s2;
    plg[2][3] = 15.0 * s2 * c;
    plg[2][4] = 7.5 * (7.0 * c2 - 1.0) * s2;
    plg[2][5] = 3.0 * c * plg[2][4] - 2.0 * plg[2][3];
    plg[2][6] = (11.0 * c * plg[2][5] - 7.0 * plg[2][4]) / 4.0;
    plg[2][7] = (13.0 * c * plg[2][6] - 8.0 * plg[2][5]) / 5.0;
    plg[3][3] = 15.0 * s2 * s;
    plg[3][4] = 105.0 * s2 * s * c;
    plg[3][5] = (9.0 * c * plg[3][4] - 7. * plg[3][3]) / 2.0;
    plg[3][6] = (11.0 * c * plg[3][5] - 8. * plg[3][4]) / 3.0;
}

/// The local time harmonics at `tloc` hours.
fn local_time(tloc: f64) -> LocalTime {
    LocalTime {
        s1: libm::sin(HR * tloc),
        c1: libm::cos(HR * tloc),
        s2: libm::sin(2.0 * HR * tloc),
        c2: libm::cos(2.0 * HR * tloc),
        s3: libm::sin(3.0 * HR * tloc),
        c3: libm::cos(3.0 * HR * tloc),
    }
}

/// `sum(|sw[i+1]| * t[i])` over the 14 terms, from `start`.
fn switched_sum(start: f64, sw: &[f64; 24], t: &[f64; 15]) -> f64 {
    sw.iter()
        .skip(1)
        .zip(t.iter())
        .take(14)
        .fold(start, |acc, (s, ti)| acc + s.abs() * *ti)
}

/// Seasonal cosines `cd32`, `cd18`, `cd14`, `cd39`.
fn seasons(p: &[f64], doy: f64) -> [f64; 4] {
    let at = |i: usize| p.get(i).copied().unwrap_or(f64::NAN);
    [
        libm::cos(DR * (doy - at(31))),
        libm::cos(2.0 * DR * (doy - at(17))),
        libm::cos(DR * (doy - at(13))),
        libm::cos(2.0 * DR * (doy - at(38))),
    ]
}

/// `globe7`: the upper-thermosphere `G(L)` of parameter set `p`. Leaves
/// the Legendre polynomials, local time harmonics, `dfa` and the magnetic
/// activity functions in `ctx` for [`glob7s`].
pub(super) fn globe7(p: &[f64; 150], input: &Input, ctx: &mut Context) -> f64 {
    let mut t = [0.0f64; 15];
    let tloc = input.lst;
    let (sw, swc) = (ctx.sw, ctx.swc);

    let c = libm::sin(input.g_lat * DGTR);
    let s = libm::cos(input.g_lat * DGTR);
    legendre(&mut ctx.plg, c, s);
    if sw[7] != 0.0 || sw[8] != 0.0 || sw[14] != 0.0 {
        ctx.tloc = local_time(tloc);
    }
    let [cd32, cd18, cd14, cd39] = seasons(p, f64::from(input.doy));

    // F10.7 effect.
    let df = input.f107 - input.f107a;
    let dfa = input.f107a - 150.0;
    ctx.dfa = dfa;
    let plg = &ctx.plg;
    let lt = ctx.tloc;
    t[0] = p[19] * df * (1.0 + p[59] * dfa)
        + p[20] * df * df
        + p[21] * dfa
        + p[29] * libm::pow(dfa, 2.0);
    let f1 = 1.0 + (p[47] * dfa + p[19] * df + p[20] * df * df) * swc[1];
    let f2 = 1.0 + (p[49] * dfa + p[19] * df + p[20] * df * df) * swc[1];

    // Time independent.
    t[1] = (p[1] * plg[0][2] + p[2] * plg[0][4] + p[22] * plg[0][6])
        + (p[14] * plg[0][2]) * dfa * swc[1]
        + p[26] * plg[0][1];
    // Symmetrical annual.
    t[2] = p[18] * cd32;
    // Symmetrical semiannual.
    t[3] = (p[15] + p[16] * plg[0][2]) * cd18;
    // Asymmetrical annual.
    t[4] = f1 * (p[9] * plg[0][1] + p[10] * plg[0][3]) * cd14;
    // Asymmetrical semiannual.
    t[5] = p[37] * plg[0][1] * cd39;

    // Diurnal.
    if sw[7] != 0.0 {
        let t71 = (p[11] * plg[1][2]) * cd14 * swc[5];
        let t72 = (p[12] * plg[1][2]) * cd14 * swc[5];
        t[6] = f2
            * ((p[3] * plg[1][1] + p[4] * plg[1][3] + p[27] * plg[1][5] + t71) * lt.c1
                + (p[6] * plg[1][1] + p[7] * plg[1][3] + p[28] * plg[1][5] + t72) * lt.s1);
    }
    // Semidiurnal.
    if sw[8] != 0.0 {
        let t81 = (p[23] * plg[2][3] + p[35] * plg[2][5]) * cd14 * swc[5];
        let t82 = (p[33] * plg[2][3] + p[36] * plg[2][5]) * cd14 * swc[5];
        t[7] = f2
            * ((p[5] * plg[2][2] + p[41] * plg[2][4] + t81) * lt.c2
                + (p[8] * plg[2][2] + p[42] * plg[2][4] + t82) * lt.s2);
    }
    // Terdiurnal.
    if sw[14] != 0.0 {
        t[13] = f2
            * ((p[39] * plg[3][3] + (p[93] * plg[3][4] + p[46] * plg[3][6]) * cd14 * swc[5])
                * lt.s3
                + (p[40] * plg[3][3] + (p[94] * plg[3][4] + p[48] * plg[3][6]) * cd14 * swc[5])
                    * lt.c3);
    }

    magnetic_activity(p, input, ctx, cd14, &mut t);
    if (sw[10] != 0.0) && (input.g_long > -1000.0) {
        longitudinal(p, input, ctx, cd14, &mut t);
    }

    // Parameters 82, 89, 99 and 139-149 are unused.
    switched_sum(p[30], &sw, &t)
}

/// `globe7`'s magnetic activity term `t[8]`, from the daily ap or (switch
/// 9 at -1) the 3-hour ap history, leaving its activity function in `ctx`.
fn magnetic_activity(
    p: &[f64; 150],
    input: &Input,
    ctx: &mut Context,
    cd14: f64,
    t: &mut [f64; 15],
) {
    let (sw, swc) = (ctx.sw, ctx.swc);
    let plg = &ctx.plg;
    let tloc = input.lst;
    if sw[9] == -1.0 {
        let Some(ap) = input.ap_a.as_ref() else {
            return;
        };
        if p[51] != 0.0 {
            let mut exp1 = libm::exp(
                -10800.0 * libm::sqrt(p[51] * p[51])
                    / (1.0 + p[138] * (45.0 - libm::sqrt(input.g_lat * input.g_lat))),
            );
            if exp1 > 0.99999 {
                exp1 = 0.99999;
            }
            // C clamps p[24] in the shared table; every table already
            // holds it above the floor, so the clamp is local here.
            let p24 = if p[24] < 1.0E-4 { 1.0E-4 } else { p[24] };
            let apt0 = sg0(exp1, p24, p[25], ap);
            if sw[9] != 0.0 {
                t[8] = apt0
                    * (p[50]
                        + p[96] * plg[0][2]
                        + p[54] * plg[0][4]
                        + (p[125] * plg[0][1] + p[126] * plg[0][3] + p[127] * plg[0][5])
                            * cd14
                            * swc[5]
                        + (p[128] * plg[1][1] + p[129] * plg[1][3] + p[130] * plg[1][5])
                            * swc[7]
                            * libm::cos(HR * (tloc - p[131])));
            }
            ctx.apt0 = apt0;
        }
    } else {
        let apd = input.ap - 4.0;
        let mut p44 = p[43];
        let p45 = p[44];
        if p44 < 0.0 {
            p44 = 1.0E-5;
        }
        let apdf = apd + (p45 - 1.0) * (apd + (libm::exp(-p44 * apd) - 1.0) / p44);
        if sw[9] != 0.0 {
            t[8] = apdf
                * (p[32]
                    + p[45] * plg[0][2]
                    + p[34] * plg[0][4]
                    + (p[100] * plg[0][1] + p[101] * plg[0][3] + p[102] * plg[0][5])
                        * cd14
                        * swc[5]
                    + (p[121] * plg[1][1] + p[122] * plg[1][3] + p[123] * plg[1][5])
                        * swc[7]
                        * libm::cos(HR * (tloc - p[124])));
        }
        ctx.apdf = apdf;
    }
}

/// `globe7`'s longitudinal, UT and mixed UT/longitude terms `t[10..=12]`.
fn longitudinal(p: &[f64; 150], input: &Input, ctx: &Context, cd14: f64, t: &mut [f64; 15]) {
    let (sw, swc) = (&ctx.sw, &ctx.swc);
    let plg = &ctx.plg;
    let dfa = ctx.dfa;
    let (sec, g_long) = (input.sec, input.g_long);

    // Longitudinal.
    if sw[11] != 0.0 {
        t[10] = (1.0 + p[80] * dfa * swc[1])
            * ((p[64] * plg[1][2]
                + p[65] * plg[1][4]
                + p[66] * plg[1][6]
                + p[103] * plg[1][1]
                + p[104] * plg[1][3]
                + p[105] * plg[1][5]
                + swc[5] * (p[109] * plg[1][1] + p[110] * plg[1][3] + p[111] * plg[1][5]) * cd14)
                * libm::cos(DGTR * g_long)
                + (p[90] * plg[1][2]
                    + p[91] * plg[1][4]
                    + p[92] * plg[1][6]
                    + p[106] * plg[1][1]
                    + p[107] * plg[1][3]
                    + p[108] * plg[1][5]
                    + swc[5]
                        * (p[112] * plg[1][1] + p[113] * plg[1][3] + p[114] * plg[1][5])
                        * cd14)
                    * libm::sin(DGTR * g_long));
    }

    // UT and mixed UT, longitude.
    if sw[12] != 0.0 {
        t[11] = (1.0 + p[95] * plg[0][1])
            * (1.0 + p[81] * dfa * swc[1])
            * (1.0 + p[119] * plg[0][1] * swc[5] * cd14)
            * ((p[68] * plg[0][1] + p[69] * plg[0][3] + p[70] * plg[0][5])
                * libm::cos(SR * (sec - p[71])));
        t[11] += swc[11]
            * (p[76] * plg[2][3] + p[77] * plg[2][5] + p[78] * plg[2][7])
            * libm::cos(SR * (sec - p[79]) + 2.0 * DGTR * g_long)
            * (1.0 + p[137] * dfa * swc[1]);
    }

    // UT, longitude magnetic activity.
    if sw[13] != 0.0 {
        t[12] = ut_longitude_magnetic(p, input, ctx, cd14, t[12]);
    }
}

/// `globe7`'s mixed UT, longitude and magnetic activity term `t[12]`,
/// `t12` where C leaves it unset.
fn ut_longitude_magnetic(
    p: &[f64; 150],
    input: &Input,
    ctx: &Context,
    cd14: f64,
    mut t12: f64,
) -> f64 {
    let (sw, swc) = (&ctx.sw, &ctx.swc);
    let plg = &ctx.plg;
    let (sec, g_long) = (input.sec, input.g_long);
    if sw[9] == -1.0 {
        if p[51] != 0.0 {
            let apt0 = ctx.apt0;
            t12 = apt0
                * swc[11]
                * (1. + p[132] * plg[0][1])
                * ((p[52] * plg[1][2] + p[98] * plg[1][4] + p[67] * plg[1][6])
                    * libm::cos(DGTR * (g_long - p[97])))
                + apt0
                    * swc[11]
                    * swc[5]
                    * (p[133] * plg[1][1] + p[134] * plg[1][3] + p[135] * plg[1][5])
                    * cd14
                    * libm::cos(DGTR * (g_long - p[136]))
                + apt0
                    * swc[12]
                    * (p[55] * plg[0][1] + p[56] * plg[0][3] + p[57] * plg[0][5])
                    * libm::cos(SR * (sec - p[58]));
        }
    } else {
        let apdf = ctx.apdf;
        t12 = apdf
            * swc[11]
            * (1.0 + p[120] * plg[0][1])
            * ((p[60] * plg[1][2] + p[61] * plg[1][4] + p[62] * plg[1][6])
                * libm::cos(DGTR * (g_long - p[63])))
            + apdf
                * swc[11]
                * swc[5]
                * (p[115] * plg[1][1] + p[116] * plg[1][3] + p[117] * plg[1][5])
                * cd14
                * libm::cos(DGTR * (g_long - p[118]))
            + apdf
                * swc[12]
                * (p[83] * plg[0][1] + p[84] * plg[0][3] + p[85] * plg[0][5])
                * libm::cos(SR * (sec - p[75]));
    }
    t12
}

/// `glob7s`: the lower-atmosphere `G(L)` of parameter set `p`, from the
/// Legendre polynomials, local time harmonics and activity functions the
/// last [`globe7`] left in `ctx`.
pub(super) fn glob7s(p: &[f64; 100], input: &Input, ctx: &Context) -> f64 {
    let pset = 2.0;
    let mut t = [0.0f64; 15];
    let (sw, swc) = (&ctx.sw, &ctx.swc);
    let plg = &ctx.plg;
    let lt = ctx.tloc;
    let doy = f64::from(input.doy);
    // Confirm the parameter set; C stores pset over a zero p[99].
    let p99 = if p[99] == 0.0 { pset } else { p[99] };
    if p99 != pset {
        warn!("NRLMSISE-00: wrong parameter set for glob7s");
        return -1.0;
    }
    let [cd32, cd18, cd14, cd39] = seasons(p, doy);

    // F10.7.
    t[0] = p[21] * ctx.dfa;
    // Time independent.
    t[1] = p[1] * plg[0][2]
        + p[2] * plg[0][4]
        + p[22] * plg[0][6]
        + p[26] * plg[0][1]
        + p[14] * plg[0][3]
        + p[59] * plg[0][5];
    // Symmetrical annual.
    t[2] = (p[18] + p[47] * plg[0][2] + p[29] * plg[0][4]) * cd32;
    // Symmetrical semiannual.
    t[3] = (p[15] + p[16] * plg[0][2] + p[30] * plg[0][4]) * cd18;
    // Asymmetrical annual.
    t[4] = (p[9] * plg[0][1] + p[10] * plg[0][3] + p[20] * plg[0][5]) * cd14;
    // Asymmetrical semiannual.
    t[5] = (p[37] * plg[0][1]) * cd39;

    // Diurnal.
    if sw[7] != 0.0 {
        let t71 = p[11] * plg[1][2] * cd14 * swc[5];
        let t72 = p[12] * plg[1][2] * cd14 * swc[5];
        t[6] = (p[3] * plg[1][1] + p[4] * plg[1][3] + t71) * lt.c1
            + (p[6] * plg[1][1] + p[7] * plg[1][3] + t72) * lt.s1;
    }
    // Semidiurnal.
    if sw[8] != 0.0 {
        let t81 = (p[23] * plg[2][3] + p[35] * plg[2][5]) * cd14 * swc[5];
        let t82 = (p[33] * plg[2][3] + p[36] * plg[2][5]) * cd14 * swc[5];
        t[7] = (p[5] * plg[2][2] + p[41] * plg[2][4] + t81) * lt.c2
            + (p[8] * plg[2][2] + p[42] * plg[2][4] + t82) * lt.s2;
    }
    // Terdiurnal.
    if sw[14] != 0.0 {
        t[13] = p[39] * plg[3][3] * lt.s3 + p[40] * plg[3][3] * lt.c3;
    }

    // Magnetic activity.
    if sw[9] != 0.0 {
        if sw[9] == 1.0 {
            t[8] = ctx.apdf * (p[32] + p[45] * plg[0][2] * swc[2]);
        }
        if sw[9] == -1.0 {
            t[8] = p[50] * ctx.apt0 + p[96] * plg[0][2] * ctx.apt0 * swc[2];
        }
    }

    // Longitudinal.
    let no_longitude = input.g_long <= -1000.0;
    if !((sw[10] == 0.0) || (sw[11] == 0.0) || no_longitude) {
        t[10] = glob7s_longitudinal(p, input, ctx, doy);
    }
    switched_sum(0.0, sw, &t)
}

/// `glob7s`'s longitudinal term `t[10]`.
fn glob7s_longitudinal(p: &[f64; 100], input: &Input, ctx: &Context, doy: f64) -> f64 {
    let swc = &ctx.swc;
    let plg = &ctx.plg;
    (1.0 + plg[0][1]
        * (p[80] * swc[5] * libm::cos(DR * (doy - p[81]))
            + p[85] * swc[6] * libm::cos(2.0 * DR * (doy - p[86])))
        + p[83] * swc[3] * libm::cos(DR * (doy - p[84]))
        + p[87] * swc[4] * libm::cos(2.0 * DR * (doy - p[88])))
        * ((p[64] * plg[1][2]
            + p[65] * plg[1][4]
            + p[66] * plg[1][6]
            + p[74] * plg[1][1]
            + p[75] * plg[1][3]
            + p[76] * plg[1][5])
            * libm::cos(DGTR * input.g_long)
            + (p[90] * plg[1][2]
                + p[91] * plg[1][4]
                + p[92] * plg[1][6]
                + p[77] * plg[1][1]
                + p[78] * plg[1][3]
                + p[79] * plg[1][5])
                * libm::sin(DGTR * input.g_long))
}
