// SPDX-License-Identifier: LGPL-2.1-or-later
//! The spread of the simulators as the yardstick. The simulators are
//! anonymous and one of them is probably JSBSim, so none is taken as truth:
//! at each sample the simulators that recorded a variable span `[lo, hi]`,
//! and a candidate sample passes when it lies inside that interval,
//! widened only by the recordings' printed precision (ten significant
//! digits of the variable's largest magnitude over the run, or of 1 when
//! smaller).

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
    /// Time keys of the samples outside it, in order.
    pub outside: Vec<i64>,
}

impl ColumnStats {
    /// Whether every sample lies inside the interval.
    #[must_use]
    pub fn passes(&self) -> bool {
        self.outside.is_empty()
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
        outside: Vec::new(),
    };
    let magnitude = simulators
        .iter()
        .filter_map(|s| s.values.get(column.nasa))
        .flat_map(|series| series.values())
        .fold(0.0_f64, |m, v| m.max(v.abs()));
    let floor = PRINTED_PRECISION * magnitude.max(1.0);
    let mut values = Vec::with_capacity(simulators.len());
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
        } else {
            stats.outside.push(key);
        }
    }
    (stats.samples > 0).then_some(stats)
}
