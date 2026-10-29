// SPDX-License-Identifier: LGPL-2.1-or-later
// NRLMSISE-00 by Mike Picone, Alan Hedin and Doug Drob (NRL); C release
// Copyright (C) Dominik Brodowski.
// Derived from JSBSim src/models/atmosphere/MSIS/nrlmsise-00.c (splini,
// splint, spline).
//
//! Cubic splines of the inverse temperature profiles, adapted by the
//! model's authors from Numerical Recipes (Press et al.).
//!
//! The node arrays hold four or five entries and every index stays inside
//! them; an index past the end would read NaN rather than panic.

use tracing::warn;

/// `a[i]`, NaN past the end.
fn at(a: &[f64], i: usize) -> f64 {
    a.get(i).copied().unwrap_or(f64::NAN)
}

/// `a[i] = v`; nothing past the end.
fn set(a: &mut [f64], i: usize, v: f64) {
    if let Some(slot) = a.get_mut(i) {
        *slot = v;
    }
}

/// `splini`: integral of the cubic spline (`xa`, `ya`, second derivatives
/// `y2a`) from `xa[0]` to `x`.
pub(super) fn splini(xa: &[f64], ya: &[f64], y2a: &[f64], x: f64) -> f64 {
    let n = xa.len();
    let mut yi = 0.0;
    let mut klo = 0;
    let mut khi = 1;
    while (x > at(xa, klo)) && (khi < n) {
        let xx = if khi < (n - 1) {
            if x < at(xa, khi) { x } else { at(xa, khi) }
        } else {
            x
        };
        let h = at(xa, khi) - at(xa, klo);
        let a = (at(xa, khi) - xx) / h;
        let b = (xx - at(xa, klo)) / h;
        let a2 = a * a;
        let b2 = b * b;
        yi += ((1.0 - a2) * at(ya, klo) / 2.0
            + b2 * at(ya, khi) / 2.0
            + ((-(1.0 + a2 * a2) / 4.0 + a2 / 2.0) * at(y2a, klo)
                + (b2 * b2 / 4.0 - b2 / 2.0) * at(y2a, khi))
                * h
                * h
                / 6.0)
            * h;
        klo += 1;
        khi += 1;
    }
    yi
}

/// `splint`: the cubic spline (`xa`, `ya`, second derivatives `y2a`) at
/// `x`.
pub(super) fn splint(xa: &[f64], ya: &[f64], y2a: &[f64], x: f64) -> f64 {
    let mut klo = 0;
    let mut khi = xa.len().saturating_sub(1);
    while (khi - klo) > 1 {
        let k = (khi + klo) / 2;
        if at(xa, k) > x {
            khi = k;
        } else {
            klo = k;
        }
    }
    let h = at(xa, khi) - at(xa, klo);
    if h == 0.0 {
        warn!("NRLMSISE-00: bad XA input to splint");
    }
    let a = (at(xa, khi) - x) / h;
    let b = (x - at(xa, klo)) / h;
    a * at(ya, klo)
        + b * at(ya, khi)
        + ((a * a * a - a) * at(y2a, klo) + (b * b * b - b) * at(y2a, khi)) * h * h / 6.0
}

/// `spline`: second derivatives of the cubic spline through (`x`, `y`)
/// with end slopes `yp1` and `ypn` (from 1e30 up: natural end).
pub(super) fn spline<const N: usize>(x: &[f64; N], y: &[f64; N], yp1: f64, ypn: f64) -> [f64; N] {
    let n = N;
    let mut y2 = [0.0; N];
    let mut u = [0.0; N];
    if yp1 > 0.99E30 {
        set(&mut y2, 0, 0.0);
        set(&mut u, 0, 0.0);
    } else {
        set(&mut y2, 0, -0.5);
        set(
            &mut u,
            0,
            (3.0 / (at(x, 1) - at(x, 0))) * ((at(y, 1) - at(y, 0)) / (at(x, 1) - at(x, 0)) - yp1),
        );
    }
    for i in 1..(n - 1) {
        let sig = (at(x, i) - at(x, i - 1)) / (at(x, i + 1) - at(x, i - 1));
        let p = sig * at(&y2, i - 1) + 2.0;
        set(&mut y2, i, (sig - 1.0) / p);
        let ui = (6.0
            * ((at(y, i + 1) - at(y, i)) / (at(x, i + 1) - at(x, i))
                - (at(y, i) - at(y, i - 1)) / (at(x, i) - at(x, i - 1)))
            / (at(x, i + 1) - at(x, i - 1))
            - sig * at(&u, i - 1))
            / p;
        set(&mut u, i, ui);
    }
    let (qn, un) = if ypn > 0.99E30 {
        (0.0, 0.0)
    } else {
        (
            0.5,
            (3.0 / (at(x, n - 1) - at(x, n - 2)))
                * (ypn - (at(y, n - 1) - at(y, n - 2)) / (at(x, n - 1) - at(x, n - 2))),
        )
    };
    let last = (un - qn * at(&u, n - 2)) / (qn * at(&y2, n - 2) + 1.0);
    set(&mut y2, n - 1, last);
    for k in (0..(n - 1)).rev() {
        let yk = at(&y2, k) * at(&y2, k + 1) + at(&u, k);
        set(&mut y2, k, yk);
    }
    y2
}
