// SPDX-License-Identifier: LGPL-2.1-or-later
//! Where a candidate sits within the simulators' own spread, reported as
//! information beside the strict envelope. The simulators disagree among
//! themselves by more than the envelope admits for an independent one:
//! each simulator is also compared, leave-one-out, with the envelope of the
//! others.

use tracing::info;

use super::data::{Column, Simulator, time_key};
use super::envelope::{compare_simulators, unwrap};

/// How far a candidate lies from its nearest simulator, against how far
/// the two closest simulators lie from each other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpreadRatio {
    /// Largest distance over time to the simulator it is nearest to.
    pub nearest: f64,
    /// Largest distance over time between the two closest simulators.
    pub closest_pair: f64,
}

impl SpreadRatio {
    /// R: at most 1 when the candidate agrees with some simulator at least
    /// as well as the two closest simulators agree with each other.
    #[must_use]
    pub fn ratio(&self) -> f64 {
        if self.nearest == 0.0 {
            0.0
        } else {
            self.nearest / self.closest_pair
        }
    }
}

/// `r` to two significant figures.
#[must_use]
pub fn two_figures(r: f64) -> String {
    if !r.is_finite() || r == 0.0 {
        return format!("{r}");
    }
    let exponent = r.abs().log10().floor();
    if !(-3.0..=4.0).contains(&exponent) {
        return format!("{r:.1e}");
    }
    if exponent >= 2.0 {
        let unit = 10f64.powi(exponent as i32 - 1);
        return format!("{}", (r / unit).round() * unit);
    }
    let decimals = (1.0 - exponent) as usize;
    format!("{r:.decimals$}")
}

/// Largest distance between `samples` and `sim`'s recording of `column`
/// at the samples' times; `None` when they share no time.
fn max_distance(column: &Column, samples: &[(f64, f64)], sim: &Simulator) -> Option<f64> {
    let mut max: Option<f64> = None;
    for &(t, x) in samples {
        if let Some(y) = sim.value(column.nasa, time_key(t)) {
            let d = (unwrap(x, y, column.period) - y).abs();
            max = Some(max.map_or(d, |m| m.max(d)));
        }
    }
    max
}

/// `sim`'s recording of `column` at `times`, where it has one.
fn recording_at(column: &Column, sim: &Simulator, times: &[f64]) -> Vec<(f64, f64)> {
    times
        .iter()
        .filter_map(|&t| Some((t, sim.value(column.nasa, time_key(t))?)))
        .collect()
}

/// R of `samples`, (time, value in the variable's unit), over their times;
/// `None` unless two simulators recorded the variable at shared times.
#[must_use]
pub fn spread_ratio(
    column: &Column,
    simulators: &[Simulator],
    samples: &[(f64, f64)],
) -> Option<SpreadRatio> {
    let recorded: Vec<&Simulator> = simulators
        .iter()
        .filter(|s| s.values.contains_key(column.nasa))
        .collect();
    let times: Vec<f64> = samples.iter().map(|&(t, _)| t).collect();
    let mut closest_pair: Option<f64> = None;
    for (i, a) in recorded.iter().enumerate() {
        let a_samples = recording_at(column, a, &times);
        for b in recorded.iter().skip(i + 1) {
            if let Some(d) = max_distance(column, &a_samples, b) {
                closest_pair = Some(closest_pair.map_or(d, |m| m.min(d)));
            }
        }
    }
    let nearest = recorded
        .iter()
        .filter_map(|s| max_distance(column, samples, s))
        .reduce(f64::min)?;
    Some(SpreadRatio {
        nearest,
        closest_pair: closest_pair?,
    })
}

/// For each simulator recording `column`, whether its recording at
/// `times` lies outside the envelope of the others somewhere; only
/// simulators with at least two others to compare with are listed.
#[must_use]
pub fn leave_one_out(
    column: &Column,
    simulators: &[Simulator],
    times: &[f64],
) -> Vec<(String, bool)> {
    let mut verdicts = Vec::new();
    for (i, sim) in simulators.iter().enumerate() {
        if !sim.values.contains_key(column.nasa) {
            continue;
        }
        let others: Vec<&Simulator> = simulators
            .iter()
            .enumerate()
            .filter_map(|(j, s)| (j != i).then_some(s))
            .collect();
        let samples = recording_at(column, sim, times);
        if let Some(stats) = compare_simulators(column, &others, &samples)
            && stats.simulators >= 2
        {
            verdicts.push((sim.name.clone(), !stats.passes()));
        }
    }
    verdicts
}

/// Logs, per simulator, on how many of `columns` it lies outside the
/// envelope of the others at `times`.
pub(super) fn report_leave_one_out(
    case: &str,
    columns: &[Column],
    simulators: &[Simulator],
    times: &[f64],
) {
    for sim in simulators {
        let (mut outside, mut compared) = (0usize, 0usize);
        for column in columns {
            for (name, out) in leave_one_out(column, simulators, times) {
                if name == sim.name {
                    compared = compared.wrapping_add(1);
                    outside = outside.wrapping_add(usize::from(out));
                }
            }
        }
        if compared > 0 {
            info!(
                "{case} leave-one-out: {} outside the other simulators' envelope on {outside}/{compared} variables",
                sim.name
            );
        }
    }
}

#[cfg(test)]
mod tests;
