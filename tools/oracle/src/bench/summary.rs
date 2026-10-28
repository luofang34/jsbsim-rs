// SPDX-License-Identifier: LGPL-2.1-or-later
//! Reducing repeated timings to the numbers a report carries.

/// Median and range of a set of timings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Summary {
    /// Middle value; the mean of the two middle values for an even count.
    pub median: f64,
    /// Fastest repeat.
    pub min: f64,
    /// Slowest repeat.
    pub max: f64,
}

impl Summary {
    /// Summarizes `samples`; `None` when there are none.
    #[must_use]
    pub fn of(samples: &[f64]) -> Option<Self> {
        let mut sorted = samples.to_vec();
        sorted.sort_by(f64::total_cmp);
        let min = *sorted.first()?;
        let max = *sorted.last()?;
        let mid = sorted.len() / 2;
        let upper = *sorted.get(mid)?;
        let median = if sorted.len().is_multiple_of(2) {
            f64::midpoint(*sorted.get(mid.wrapping_sub(1))?, upper)
        } else {
            upper
        };
        Some(Self { median, min, max })
    }
}

/// How many times longer Rust takes than C++ per frame.
#[must_use]
pub fn ratio(rust_ns: f64, cpp_ns: f64) -> f64 {
    rust_ns / cpp_ns
}

/// Frames per second at `ns_per_frame`.
#[must_use]
pub fn fps(ns_per_frame: f64) -> f64 {
    1e9 / ns_per_frame
}
