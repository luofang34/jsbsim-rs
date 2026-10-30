// SPDX-License-Identifier: LGPL-2.1-or-later
//! Samples where Rust and C++ fall on opposite sides of the pass threshold
//! only because the gate builds' sin and cos differ in the last digit.
//! The sincos gate shows the port bit-identical with C++ when both use the
//! same maths library; under the gate builds the case's parity bound
//! covers their difference. A sample whose verdict differs is accepted
//! only when C++ lies within that bound of the threshold and Rust within
//! it of C++ at that sample, so a real port difference cannot hide behind
//! a C++ value near the threshold. A case allows at most its manifest's
//! `max_threshold_flips` of them.

use super::data::{Column, Simulator, time_key};
use super::envelope::{excursion, floor};
use crate::Series;

/// How far the sample `x` (in the variable's unit) at time key `key`
/// lies from the threshold between inside and outside the simulators'
/// interval, printed precision included.
#[must_use]
pub fn threshold_distance(
    column: &Column,
    simulators: &[Simulator],
    key: i64,
    x: f64,
) -> Option<f64> {
    let simulators: Vec<&Simulator> = simulators.iter().collect();
    let (excursion, _) = excursion(column, &simulators, key, x)?;
    Some((excursion - floor(column, &simulators)).abs())
}

/// One accepted flip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flip {
    /// Time key of the sample.
    pub key: i64,
    /// C++'s distance to the threshold, in the property's unit.
    pub cpp_to_threshold: f64,
    /// |Rust - C++| at the sample.
    pub rust_to_cpp: f64,
    /// The case's bound, in the property's unit.
    pub bound: f64,
}

/// `property` of `series` at time key `key`.
fn at(series: &Series, index: usize, key: i64) -> Option<f64> {
    series
        .rows
        .iter()
        .find(|r| r.first().is_some_and(|&t| time_key(t) == key))
        .and_then(|r| r.get(index).copied())
}

/// The samples on which `cpp_outside` and `rust_outside` disagree, when
/// every one of them is accepted under `rel` (the case's parity bound,
/// relative to the property's largest C++ magnitude or 1); `None` when
/// any is not.
#[must_use]
pub fn accepted_flips(
    column: &Column,
    simulators: &[Simulator],
    (cpp, rust): (&Series, &Series),
    (cpp_outside, rust_outside): (&[i64], &[i64]),
    rel: f64,
) -> Option<Vec<Flip>> {
    let ci = cpp.columns.iter().position(|c| c == column.property)?;
    let ri = rust.columns.iter().position(|c| c == column.property)?;
    let magnitude = cpp
        .rows
        .iter()
        .filter_map(|r| r.get(ci))
        .fold(1.0_f64, |m, v| m.max(v.abs()));
    let bound = rel * magnitude;
    let differing = cpp_outside
        .iter()
        .filter(|k| !rust_outside.contains(k))
        .chain(rust_outside.iter().filter(|k| !cpp_outside.contains(k)));
    let mut flips = Vec::new();
    for &key in differing {
        let (c, r) = (at(cpp, ci, key)?, at(rust, ri, key)?);
        let distance =
            threshold_distance(column, simulators, key, c * column.scale)? / column.scale.abs();
        let rust_to_cpp = (r - c).abs();
        if !(distance <= bound && rust_to_cpp <= bound) {
            return None;
        }
        flips.push(Flip {
            key,
            cpp_to_threshold: distance,
            rust_to_cpp,
            bound,
        });
    }
    Some(flips)
}

#[cfg(test)]
mod tests;
