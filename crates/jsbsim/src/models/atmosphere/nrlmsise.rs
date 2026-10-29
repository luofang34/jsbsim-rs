// SPDX-License-Identifier: LGPL-2.1-or-later
// NRLMSISE-00 by Mike Picone, Alan Hedin and Doug Drob (NRL); C release
// Copyright (C) Dominik Brodowski.
// Derived from JSBSim src/models/atmosphere/MSIS/nrlmsise-00.{c,h} (gtd7).
//
//! The NRLMSISE-00 empirical atmosphere model, as far as JSBSim's `FGMSIS`
//! reaches it: `gtd7`, the neutral temperature and species densities from
//! the ground to the exosphere.
//!
//! The C release keeps its intermediate results in file-scope variables;
//! here they live in a context built afresh by every [`gtd7`] call. No
//! value flows from one call to the next in C either, with one exception:
//! with switch 15 (departures from diffusive equilibrium) off, C's `gtd7`
//! below 72.5 km reads the N2 mixed density `dm28` left by an earlier call
//! (zero before the first); the port always reads zero there, as C does on
//! its first call. JSBSim turns every switch on.

mod context;
mod globe;
mod lower_coefficients;
mod profile;
mod spline;
#[cfg(test)]
mod tests;
mod thermosphere;
mod upper_coefficients;

use context::Context;
use lower_coefficients::{PAVGM, PDM, PMA};

/// `struct nrlmsise_flags`'s caller-set part: the 24 model switches.
///
/// Switch 0 selects the output units (1: m⁻³ and kg/m³, 0: cm⁻³ and
/// g/cm³). Each other switch turns an effect off (0), on (1), or on for its
/// cross terms only (2). Switch 9 set to -1 takes the magnetic activity
/// from [`Input::ap_a`] (3-hour history) instead of [`Input::ap`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flags {
    /// `switches`.
    pub switches: [i32; 24],
}

impl Default for Flags {
    /// Every switch on, as `FGMSIS` sets them.
    fn default() -> Self {
        Self { switches: [1; 24] }
    }
}

/// `struct nrlmsise_input`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Input {
    /// `year`: ignored by the model.
    pub year: i32,
    /// `doy`: day of year.
    pub doy: i32,
    /// `sec`: seconds in day (UT).
    pub sec: f64,
    /// `alt`: altitude, km.
    pub alt: f64,
    /// `g_lat`: geodetic latitude, degrees.
    pub g_lat: f64,
    /// `g_long`: geodetic longitude, degrees.
    pub g_long: f64,
    /// `lst`: local apparent solar time, hours.
    pub lst: f64,
    /// `f107A`: 81-day average of the F10.7 flux, centred on `doy`.
    pub f107a: f64,
    /// `f107`: daily F10.7 flux of the previous day.
    pub f107: f64,
    /// `ap`: daily magnetic index.
    pub ap: f64,
    /// `ap_a->a`: the 3-hour magnetic index history, read only when switch
    /// 9 is -1.
    pub ap_a: Option<[f64; 7]>,
}

/// `struct nrlmsise_output`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Output {
    /// Number densities of He, O, N2, O2, Ar, then total mass density
    /// (without anomalous oxygen), then H, N and anomalous O.
    pub d: [f64; 9],
    /// Exospheric temperature, then temperature at altitude, K.
    pub t: [f64; 2],
}

/// Inputs `gtd7` cannot evaluate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Switch 9 asks for the 3-hour ap history, which C dereferences
    /// through a pointer that this input leaves null.
    #[error("NRLMSISE-00 switch 9 is -1 but the input has no ap_a history")]
    MissingApHistory,
}

/// `tselec`: the effective switches `sw` and the cross-term switches `swc`.
fn tselec(flags: &Flags) -> ([f64; 24], [f64; 24]) {
    let mut sw = [0.0; 24];
    let mut swc = [0.0; 24];
    for (i, ((s, sc), &switch)) in sw
        .iter_mut()
        .zip(swc.iter_mut())
        .zip(flags.switches.iter())
        .enumerate()
    {
        if i != 9 {
            *s = if switch == 1 { 1.0 } else { 0.0 };
            *sc = if switch > 0 { 1.0 } else { 0.0 };
        } else {
            *s = f64::from(switch);
            *sc = f64::from(switch);
        }
    }
    (sw, swc)
}

/// Lower mesosphere and stratosphere node altitudes, km (`zn2`).
const ZN2: [f64; 4] = [72.5, 55.0, 45.0, 32.5];
/// Lower stratosphere and troposphere node altitudes, km (`zn3`).
const ZN3: [f64; 5] = [32.5, 20.0, 15.0, 10.0, 0.0];

/// `gtd7`: neutral temperature and densities at `input`.
///
/// # Errors
///
/// [`Error::MissingApHistory`] when switch 9 is -1 and `input.ap_a` is
/// `None`.
pub fn gtd7(input: &Input, flags: &Flags) -> Result<Output, Error> {
    if flags.switches[9] == -1 && input.ap_a.is_none() {
        return Err(Error::MissingApHistory);
    }
    let (sw, swc) = tselec(flags);
    // Latitude variation of gravity (none for sw[2]=0).
    let xlat = if sw[2] == 0.0 { 45.0 } else { input.g_lat };
    let mut ctx = Context::new(sw, swc, profile::glatf(xlat));

    // Thermosphere and mesosphere, evaluated no lower than zn2[0].
    let altt = if input.alt > ZN2[0] {
        input.alt
    } else {
        ZN2[0]
    };
    let upper_input = Input {
        alt: altt,
        ..*input
    };
    let soutput = thermosphere::gts7(&upper_input, &mut ctx);
    // Metric adjustment.
    let dm28m = if sw[0] != 0.0 {
        ctx.dm28 * 1.0E6
    } else {
        ctx.dm28
    };
    let mut output = Output {
        d: [0.0; 9],
        t: soutput.t,
    };
    if input.alt >= ZN2[0] {
        output.d = soutput.d;
        return Ok(output);
    }
    middle_nodes(input, &mut ctx);
    lower_atmosphere(input, &ctx, &soutput, dm28m, &mut output);
    Ok(output)
}

/// Node temperatures and end gradients of the lower mesosphere and upper
/// stratosphere, and below zn3[0] of the lower stratosphere and
/// troposphere: inverse temperatures linear in spherical harmonics.
fn middle_nodes(input: &Input, ctx: &mut Context) {
    let sw = ctx.sw;
    ctx.n2.tg[0] = ctx.n1.tg[1];
    ctx.n2.t[0] = ctx.n1.t[4];
    ctx.n2.t[1] = PMA[0][0] * PAVGM[0] / (1.0 - sw[20] * globe::glob7s(&PMA[0], input, ctx));
    ctx.n2.t[2] = PMA[1][0] * PAVGM[1] / (1.0 - sw[20] * globe::glob7s(&PMA[1], input, ctx));
    ctx.n2.t[3] =
        PMA[2][0] * PAVGM[2] / (1.0 - sw[20] * sw[22] * globe::glob7s(&PMA[2], input, ctx));
    ctx.n2.tg[1] = PAVGM[8]
        * PMA[9][0]
        * (1.0 + sw[20] * sw[22] * globe::glob7s(&PMA[9], input, ctx))
        * ctx.n2.t[3]
        * ctx.n2.t[3]
        / (libm::pow(PMA[2][0] * PAVGM[2], 2.0));
    ctx.n3.t[0] = ctx.n2.t[3];

    if input.alt <= ZN3[0] {
        ctx.n3.tg[0] = ctx.n2.tg[1];
        ctx.n3.t[1] = PMA[3][0] * PAVGM[3] / (1.0 - sw[22] * globe::glob7s(&PMA[3], input, ctx));
        ctx.n3.t[2] = PMA[4][0] * PAVGM[4] / (1.0 - sw[22] * globe::glob7s(&PMA[4], input, ctx));
        ctx.n3.t[3] = PMA[5][0] * PAVGM[5] / (1.0 - sw[22] * globe::glob7s(&PMA[5], input, ctx));
        ctx.n3.t[4] = PMA[6][0] * PAVGM[6] / (1.0 - sw[22] * globe::glob7s(&PMA[6], input, ctx));
        ctx.n3.tg[1] = PMA[7][0]
            * PAVGM[7]
            * (1.0 + sw[22] * globe::glob7s(&PMA[7], input, ctx))
            * ctx.n3.t[4]
            * ctx.n3.t[4]
            / (libm::pow(PMA[6][0] * PAVGM[6], 2.0));
    }
}

/// Below zn2[0]: the mixed-atmosphere densities, blended linearly into
/// the thermospheric ones above `zmix`, and the temperature at altitude.
fn lower_atmosphere(
    input: &Input,
    ctx: &Context,
    soutput: &Output,
    dm28m: f64,
    output: &mut Output,
) {
    let zmix = 62.5;
    let xmm = PDM[2][4];
    let d = &mut output.d;

    let mut dmc = 0.0;
    if input.alt > zmix {
        dmc = 1.0 - (ZN2[0] - input.alt) / (ZN2[0] - zmix);
    }
    let dz28 = soutput.d[2];
    let mut tz = 0.0;

    // N2 density.
    let mut dmr = soutput.d[2] / dm28m - 1.0;
    d[2] = profile::densm(input.alt, dm28m, xmm, &mut tz, &ctx.n3, &ctx.n2, ctx.grav);
    d[2] *= 1.0 + dmr * dmc;

    // He density.
    dmr = soutput.d[0] / (dz28 * PDM[0][1]) - 1.0;
    d[0] = d[2] * PDM[0][1] * (1.0 + dmr * dmc);

    // O density.
    d[1] = 0.0;
    d[8] = 0.0;

    // O2 density.
    dmr = soutput.d[3] / (dz28 * PDM[3][1]) - 1.0;
    d[3] = d[2] * PDM[3][1] * (1.0 + dmr * dmc);

    // Ar density.
    dmr = soutput.d[4] / (dz28 * PDM[4][1]) - 1.0;
    d[4] = d[2] * PDM[4][1] * (1.0 + dmr * dmc);

    // Hydrogen and atomic nitrogen densities.
    d[6] = 0.0;
    d[7] = 0.0;

    // Total mass density.
    d[5] = 1.66E-24
        * (4.0 * d[0] + 16.0 * d[1] + 28.0 * d[2] + 32.0 * d[3] + 40.0 * d[4] + d[6] + 14.0 * d[7]);
    if ctx.sw[0] != 0.0 {
        d[5] /= 1000.0;
    }

    // Temperature at altitude.
    profile::densm(input.alt, 1.0, 0.0, &mut tz, &ctx.n3, &ctx.n2, ctx.grav);
    output.t[1] = tz;
}
