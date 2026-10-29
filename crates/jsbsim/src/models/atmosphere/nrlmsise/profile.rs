// SPDX-License-Identifier: LGPL-2.1-or-later
// NRLMSISE-00 by Mike Picone, Alan Hedin and Doug Drob (NRL); C release
// Copyright (C) Dominik Brodowski.
// Derived from JSBSim src/models/atmosphere/MSIS/nrlmsise-00.c (glatf,
// ccor, ccor2, scalh, dnet, zeta, densm, densu).
//
//! Temperature and density profiles: the Bates profile of the upper
//! thermosphere, the spline profiles below it, and the turbopause and
//! chemistry corrections.

use tracing::warn;

use super::context::{Gravity, Nodes};
use super::spline::{spline, splini, splint};

/// Gas constant in the model's units.
const RGAS: f64 = 831.4;

/// `glatf`: gravity and effective Earth radius at latitude `lat`.
pub(super) fn glatf(lat: f64) -> Gravity {
    let dgtr = 1.74533E-2;
    let c2 = libm::cos(2.0 * dgtr * lat);
    let gv = 980.616 * (1.0 - 0.0026373 * c2);
    let reff = 2.0 * gv / (3.085462E-6 + 2.27E-9 * c2) * 1.0E-5;
    Gravity {
        gsurf: gv,
        re: reff,
    }
}

/// `ccor`: chemistry and dissociation correction for target ratio `r`,
/// transition scale length `h1` and half-ratio altitude `zh`.
pub(super) fn ccor(alt: f64, r: f64, h1: f64, zh: f64) -> f64 {
    let mut e = (alt - zh) / h1;
    if e > 70.0 {
        return libm::exp(0.0);
    }
    if e < -70.0 {
        return libm::exp(r);
    }
    let ex = libm::exp(e);
    e = r / (1.0 + ex);
    libm::exp(e)
}

/// `ccor2`: [`ccor`] with two transition scale lengths.
pub(super) fn ccor2(alt: f64, r: f64, h1: f64, zh: f64, h2: f64) -> f64 {
    let e1 = (alt - zh) / h1;
    let e2 = (alt - zh) / h2;
    if (e1 > 70.0) || (e2 > 70.0) {
        return libm::exp(0.0);
    }
    if (e1 < -70.0) && (e2 < -70.0) {
        return libm::exp(r);
    }
    let ex1 = libm::exp(e1);
    let ex2 = libm::exp(e2);
    let ccor2v = r / (1.0 + 0.5 * (ex1 + ex2));
    libm::exp(ccor2v)
}

/// `scalh`: scale height at `alt` of molecular weight `xm` at `temp`.
pub(super) fn scalh(alt: f64, xm: f64, temp: f64, grav: Gravity) -> f64 {
    let mut g = grav.gsurf / (libm::pow(1.0 + alt / grav.re, 2.0));
    g = RGAS * temp / (g * xm);
    g
}

/// `dnet`: turbopause correction combining the diffusive density `dd` and
/// the fully mixed density `dm` with transition scale length `zhm`, mixed
/// molecular weight `xmm` and species weight `xm`.
pub(super) fn dnet(mut dd: f64, dm: f64, zhm: f64, xmm: f64, xm: f64) -> f64 {
    let a = zhm / (xmm - xm);
    let both_positive = (dm > 0.0) && (dd > 0.0);
    if !both_positive {
        warn!("NRLMSISE-00: dnet log error {dm:e} {dd:e} {xm:e}");
        if (dd == 0.0) && (dm == 0.0) {
            dd = 1.0;
        }
        if dm == 0.0 {
            return dd;
        }
        if dd == 0.0 {
            return dm;
        }
    }
    let ylog = a * libm::log(dm / dd);
    if ylog < -10.0 {
        return dd;
    }
    if ylog > 10.0 {
        return dm;
    }
    dd * libm::pow(1.0 + libm::exp(ylog), 1.0 / a)
}

/// `zeta`: geopotential altitude difference of `zz` from `zl`.
fn zeta(zz: f64, zl: f64, re: f64) -> f64 {
    (zz - zl) * (re + zl) / (re + zz)
}

/// One spline segment of `densm`: temperature at `z` into `tz` and, for a
/// species weight `xm` other than zero, `density` carried down to `z`.
fn densm_segment<const N: usize>(
    z: f64,
    nodes: &Nodes<N>,
    xm: f64,
    grav: Gravity,
    density: f64,
    tz: &mut f64,
) -> f64 {
    let re = grav.re;
    let z1 = nodes.z.first().copied().unwrap_or(f64::NAN);
    let z2 = nodes.z.last().copied().unwrap_or(f64::NAN);
    let t1 = nodes.t.first().copied().unwrap_or(f64::NAN);
    let t2 = nodes.t.last().copied().unwrap_or(f64::NAN);
    let zg = zeta(z, z1, re);
    let zgdif = zeta(z2, z1, re);

    // Spline nodes.
    let mut xs = [0.0; N];
    let mut ys = [0.0; N];
    for (((x, y), &zn), &tn) in xs.iter_mut().zip(ys.iter_mut()).zip(&nodes.z).zip(&nodes.t) {
        *x = zeta(zn, z1, re) / zgdif;
        *y = 1.0 / tn;
    }
    let yd1 = -nodes.tg[0] / (t1 * t1) * zgdif;
    let yd2 = -nodes.tg[1] / (t2 * t2) * zgdif * (libm::pow((re + z2) / (re + z1), 2.0));

    // Spline coefficients, then temperature at altitude.
    let y2out = spline(&xs, &ys, yd1, yd2);
    let x = zg / zgdif;
    let y = splint(&xs, &ys, &y2out, x);
    *tz = 1.0 / y;
    if xm == 0.0 {
        return density;
    }
    let glb = grav.gsurf / (libm::pow(1.0 + z1 / re, 2.0));
    let gamm = xm * glb * zgdif / RGAS;

    // Integrated temperature profile, then density at altitude.
    let yi = splini(&xs, &ys, &y2out, x);
    let mut expl = gamm * yi;
    if expl > 50.0 {
        expl = 50.0;
    }
    density * (t1 / *tz) * libm::exp(-expl)
}

/// `densm`: temperature (`xm` zero, also left in `tz`) or density of
/// species weight `xm` from `d0` at zn2[0], below zn2[0].
pub(super) fn densm(
    alt: f64,
    d0: f64,
    xm: f64,
    tz: &mut f64,
    n3: &Nodes<5>,
    n2: &Nodes<4>,
    grav: Gravity,
) -> f64 {
    let mut densm_tmp = d0;
    if alt > n2.z[0] {
        return if xm == 0.0 { *tz } else { d0 };
    }

    // Stratosphere and mesosphere.
    let z = if alt > n2.z[3] { alt } else { n2.z[3] };
    densm_tmp = densm_segment(z, n2, xm, grav, densm_tmp, tz);
    if alt > n3.z[0] {
        return if xm == 0.0 { *tz } else { densm_tmp };
    }

    // Troposphere and stratosphere.
    densm_tmp = densm_segment(alt, n3, xm, grav, densm_tmp, tz);
    if xm == 0.0 { *tz } else { densm_tmp }
}

/// The lower thermosphere `densu` integrates through: exospheric and
/// lower-boundary temperatures, lower-boundary altitude and temperature
/// gradient ratio, and the spline nodes below `nodes.z[0]`.
#[derive(Debug, Clone, Copy)]
pub(super) struct Thermo {
    /// `tinf`, K.
    pub tinf: f64,
    /// `tlb`, K.
    pub tlb: f64,
    /// `zlb`, km.
    pub zlb: f64,
    /// `s2`.
    pub s2: f64,
    /// `zn1`, `tn1`, `tgn1`.
    pub nodes: Nodes<5>,
    /// `gsurf`, `re`.
    pub grav: Gravity,
}

/// The spline part of `densu` below za: its nodes, second derivatives,
/// abscissa, first node altitude and temperature, and geopotential span.
struct BelowZa {
    xs: [f64; 5],
    ys: [f64; 5],
    y2out: [f64; 5],
    x: f64,
    z1: f64,
    t1: f64,
    zgdif: f64,
}

/// `densu`'s temperature below za, into `tz`, from the Bates temperature
/// `ta` at za.
fn densu_below_za(th: &Thermo, alt: f64, ta: f64, tz: &mut f64) -> BelowZa {
    let re = th.grav.re;
    let za = th.nodes.z[0];
    // Temperature gradient at za from the Bates profile.
    let dta = (th.tinf - ta) * th.s2 * libm::pow((re + th.zlb) / (re + za), 2.0);
    let mut tn1 = th.nodes.t;
    let mut tgn1 = th.nodes.tg;
    tgn1[0] = dta;
    tn1[0] = ta;
    let zn1 = &th.nodes.z;
    let z = if alt > zn1[4] { alt } else { zn1[4] };
    let z1 = zn1[0];
    let z2 = zn1[4];
    let t1 = tn1[0];
    let t2 = tn1[4];
    // Geopotential difference from z1.
    let zg = zeta(z, z1, re);
    let zgdif = zeta(z2, z1, re);
    let mut xs = [0.0; 5];
    let mut ys = [0.0; 5];
    for (((x, y), &zn), &tn) in xs.iter_mut().zip(ys.iter_mut()).zip(zn1).zip(&tn1) {
        *x = zeta(zn, z1, re) / zgdif;
        *y = 1.0 / tn;
    }
    // End node derivatives.
    let yd1 = -tgn1[0] / (t1 * t1) * zgdif;
    let yd2 = -tgn1[1] / (t2 * t2) * zgdif * libm::pow((re + z2) / (re + z1), 2.0);
    let y2out = spline(&xs, &ys, yd1, yd2);
    let x = zg / zgdif;
    let y = splint(&xs, &ys, &y2out, x);
    *tz = 1.0 / y;
    BelowZa {
        xs,
        ys,
        y2out,
        x,
        z1,
        t1,
        zgdif,
    }
}

/// `densu`: temperature (`xm` zero, also left in `tz`) or density of
/// species weight `xm` with thermal diffusion coefficient `alpha`, from
/// `dlb` at the lower boundary.
pub(super) fn densu(th: &Thermo, alt: f64, dlb: f64, xm: f64, alpha: f64, tz: &mut f64) -> f64 {
    let re = th.grav.re;
    let (tinf, tlb, s2) = (th.tinf, th.tlb, th.s2);
    // Joining altitude of the Bates profile and the spline.
    let za = th.nodes.z[0];
    let z = if alt > za { alt } else { za };
    // Geopotential altitude difference from zlb.
    let zg2 = zeta(z, th.zlb, re);
    // Bates temperature.
    let tt = tinf - (tinf - tlb) * libm::exp(-s2 * zg2);
    let ta = tt;
    *tz = tt;
    let mut densu_temp = *tz;
    let below = if alt < za {
        let below = densu_below_za(th, alt, ta, tz);
        densu_temp = *tz;
        Some(below)
    } else {
        None
    };
    if xm == 0.0 {
        return densu_temp;
    }

    // Density above za.
    let mut glb = th.grav.gsurf / libm::pow(1.0 + th.zlb / re, 2.0);
    let gamma = xm * glb / (s2 * RGAS * tinf);
    let mut expl = libm::exp(-s2 * gamma * zg2);
    if expl > 50.0 {
        expl = 50.0;
    }
    if tt <= 0.0 {
        expl = 50.0;
    }
    let densa = dlb * libm::pow(tlb / tt, 1.0 + alpha + gamma) * expl;
    densu_temp = densa;
    if alt >= za {
        return densu_temp;
    }

    // Density below za. Only a NaN `alt` gets here without the spline,
    // which C then integrates over no nodes from zeroed locals.
    let (z1, zgdif, t1) = below
        .as_ref()
        .map_or((0.0, 0.0, 0.0), |b| (b.z1, b.zgdif, b.t1));
    glb = th.grav.gsurf / libm::pow(1.0 + z1 / re, 2.0);
    let gamm = xm * glb * zgdif / RGAS;
    // Integrated spline temperatures.
    let yi = below
        .as_ref()
        .map_or(0.0, |b| splini(&b.xs, &b.ys, &b.y2out, b.x));
    expl = gamm * yi;
    if expl > 50.0 {
        expl = 50.0;
    }
    if *tz <= 0.0 {
        expl = 50.0;
    }
    densu_temp * libm::pow(t1 / *tz, 1.0 + alpha) * libm::exp(-expl)
}
