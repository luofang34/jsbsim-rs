// SPDX-License-Identifier: LGPL-2.1-or-later
//! The margin a tolerance row's reason claims over its observed
//! difference, and whether a measured margin bears it out.
//!
//! A reason claims a margin in one of two phrasings, and in at most one:
//!
//! - `bound 10x that` (or `bounds 10x those`): the bound was set at ten
//!   times the observed maximum, rounded up to one significant figure.
//!   Rounding up to one figure multiplies by less than 2 (10 × 1.01e-15
//!   rounds up to 2e-14, just under 20×), so the measured margin must lie
//!   in [[`TEN_TIMES`], [`AT_MOST`]].
//! - `bound at least <m>x that`, with an integer `m` of at least
//!   [`LEAST_MARGIN`]: the bound is kept although it covers less than ten
//!   times what is observed; the measured margin must be at least `m`.
//!
//! A row claiming either margin must observe some difference; one that
//! observes none claims a margin over nothing, and is deleted when a wider
//! row (`*`, `<op>/*` or `*/<tag>`) covers the same numbers, or else
//! states that it is exact. Margins below [`LEAST_MARGIN`] fail either
//! form. Comparisons allow a relative slack of [`SLACK`], since a bound
//! such as `0.005` is not exactly ten times `0.0005` in binary.

/// Lower end of the margin `bound 10x that` claims.
pub const TEN_TIMES: f64 = 10.0;

/// Upper end of the margin `bound 10x that` claims: ten times, rounded up
/// to one significant figure, stays below twenty times.
pub const AT_MOST: f64 = 20.0;

/// Smallest margin any claim may state, and any row may have.
pub const LEAST_MARGIN: u32 = 2;

/// Relative slack on every margin comparison.
pub const SLACK: f64 = 1e-9;

const TEN_TIMES_PHRASES: [&str; 2] = ["bound 10x that", "bounds 10x those"];
const AT_LEAST_PREFIX: &str = "bound at least ";
const AT_LEAST_SUFFIX: &str = "x that";

/// The margin a reason claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claim {
    /// `bound 10x that`.
    TenTimes,
    /// `bound at least <m>x that`.
    AtLeast(u32),
}

impl Claim {
    /// The claim `reason` makes, `None` when it makes none, or why its
    /// claim cannot be read.
    pub fn parse(reason: &str) -> Result<Option<Self>, String> {
        let ten = TEN_TIMES_PHRASES.iter().any(|p| reason.contains(p));
        let at_least = at_least(reason)?;
        match (ten, at_least) {
            (true, Some(_)) => Err("claims both 10x and a lower margin".to_owned()),
            (true, None) => Ok(Some(Self::TenTimes)),
            (false, Some(m)) => Ok(Some(Self::AtLeast(m))),
            (false, None) => Ok(None),
        }
    }

    /// Why `margin`, the bound over the largest observed difference
    /// (infinite when none is observed), does not bear the claim out.
    #[must_use]
    pub fn refute(self, margin: f64) -> Option<String> {
        if margin.is_infinite() {
            return Some(
                "observes no difference: delete the row where a wider row covers it, \
                 or state that it is exact"
                    .to_owned(),
            );
        }
        let least = f64::from(LEAST_MARGIN);
        if margin.is_nan() || margin < least * (1.0 - SLACK) {
            return Some(format!(
                "the bound is {margin:.3}x the observed maximum, below the least margin \
                 {LEAST_MARGIN}x"
            ));
        }
        match self {
            Self::TenTimes if margin < TEN_TIMES * (1.0 - SLACK) => Some(format!(
                "claims 10x but the bound is {margin:.3}x the observed maximum"
            )),
            Self::TenTimes if margin > AT_MOST * (1.0 + SLACK) => Some(format!(
                "claims 10x but the bound is {margin:.3}x the observed maximum, over \
                 {AT_MOST}x: tighten it to 10x rounded up to one significant figure"
            )),
            Self::AtLeast(m) if m < LEAST_MARGIN => Some(format!(
                "claims at least {m}x, below the least margin {LEAST_MARGIN}x"
            )),
            Self::AtLeast(m) if margin < f64::from(m) * (1.0 - SLACK) => Some(format!(
                "claims at least {m}x but the bound is {margin:.3}x the observed maximum"
            )),
            _ => None,
        }
    }
}

/// The `m` of a `bound at least <m>x that` in `reason`.
fn at_least(reason: &str) -> Result<Option<u32>, String> {
    let Some((_, rest)) = reason.split_once(AT_LEAST_PREFIX) else {
        return Ok(None);
    };
    let Some((digits, _)) = rest.split_once(AT_LEAST_SUFFIX) else {
        return Err(format!(
            "'{AT_LEAST_PREFIX}' without '<m>{AT_LEAST_SUFFIX}'"
        ));
    };
    digits
        .parse::<u32>()
        .map(Some)
        .map_err(|_| format!("'{AT_LEAST_PREFIX}{digits}{AT_LEAST_SUFFIX}': not an integer margin"))
}
