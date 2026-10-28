// SPDX-License-Identifier: LGPL-2.1-or-later
//! The spread of the simulators as the yardstick. The simulators are
//! anonymous and one of them is probably JSBSim, so none is taken as truth:
//! at each sample the simulators that recorded a variable span `[lo, hi]`,
//! and a candidate passes when it is never farther outside that interval
//! than the simulators' widest spread over the run, i.e. no farther from
//! the ensemble than its members are from each other. Differences below
//! the recordings' printed precision (ten significant digits of the
//! variable's largest magnitude over the run, or of 1 when smaller) are
//! not differences. How often the candidate is strictly inside `[lo, hi]`
//! is reported too.

use super::data::{Column, Simulator, time_key};
use crate::Series;

/// Resolution of a recording printed to ten significant digits, relative
/// to the variable's largest magnitude.
const PRINTED_PRECISION: f64 = 1e-10;

/// How one candidate variable compares with the simulators.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnStats {
    /// TM variable.
    pub nasa: &'static str,
    /// Simulators that recorded it.
    pub simulators: usize,
    /// Candidate samples at which at least two simulators recorded it.
    pub samples: usize,
    /// Widest spread of the simulators.
    pub max_spread: f64,
    /// Largest distance of the candidate outside the simulators' interval.
    pub max_excursion: f64,
    /// Time of the largest excursion, s.
    pub worst_time: f64,
    /// Samples inside the interval.
    pub inside: usize,
    /// Samples no farther outside the interval than the widest spread.
    pub within_spread: usize,
}

impl ColumnStats {
    /// Whether every sample lies within the spread.
    #[must_use]
    pub fn passes(&self) -> bool {
        self.within_spread == self.samples
    }
}

/// `v` moved by whole periods to lie within half a period of `reference`.
fn unwrap(v: f64, reference: f64, period: Option<f64>) -> f64 {
    match period {
        Some(p) => v - p * ((v - reference) / p).round(),
        None => v,
    }
}

/// Compares the candidate's `column` with the simulators; `None` when the
/// candidate lacks the property or fewer than two simulators recorded the
/// variable at any candidate sample.
#[must_use]
pub fn compare_column(
    column: &Column,
    simulators: &[Simulator],
    candidate: &Series,
) -> Option<ColumnStats> {
    let index = candidate
        .columns
        .iter()
        .position(|c| c == column.property)?;
    let recorded = simulators
        .iter()
        .filter(|s| s.values.contains_key(column.nasa))
        .count();
    let mut stats = ColumnStats {
        nasa: column.nasa,
        simulators: recorded,
        samples: 0,
        max_spread: 0.0,
        max_excursion: 0.0,
        worst_time: 0.0,
        inside: 0,
        within_spread: 0,
    };
    let magnitude = simulators
        .iter()
        .filter_map(|s| s.values.get(column.nasa))
        .flat_map(|series| series.values())
        .fold(0.0_f64, |m, v| m.max(v.abs()));
    let floor = PRINTED_PRECISION * magnitude.max(1.0);
    let mut values = Vec::with_capacity(simulators.len());
    let mut excursions = Vec::with_capacity(candidate.rows.len());
    for row in &candidate.rows {
        let (Some(&t), Some(&raw)) = (row.first(), row.get(index)) else {
            continue;
        };
        let key = time_key(t);
        values.clear();
        values.extend(simulators.iter().filter_map(|s| s.value(column.nasa, key)));
        let Some(&reference) = values.first() else {
            continue;
        };
        if values.len() < 2 {
            continue;
        }
        let (lo, hi) = values
            .iter()
            .map(|v| unwrap(*v, reference, column.period))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                (lo.min(v), hi.max(v))
            });
        let x = unwrap(raw * column.scale, reference, column.period);
        let spread = hi - lo;
        let excursion = (lo - x).max(x - hi).max(0.0);
        stats.samples += 1;
        stats.max_spread = stats.max_spread.max(spread);
        if excursion > stats.max_excursion || excursion.is_nan() {
            stats.max_excursion = excursion;
            stats.worst_time = t;
        }
        if excursion <= floor {
            stats.inside += 1;
        }
        excursions.push(excursion);
    }
    let allowed = stats.max_spread + floor;
    stats.within_spread = excursions.iter().filter(|e| **e <= allowed).count();
    (stats.samples > 0).then_some(stats)
}
