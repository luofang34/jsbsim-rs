// SPDX-License-Identifier: LGPL-2.1-or-later
// FGFunction: Copyright (C) 2004 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/math/FGFunction.cpp (makeRandomGenerator, the
// random and urandom operations, and aFunc<func_t, 0>).
//
//! `<random>` and `<urandom>`: a normal deviate scaled by `stddev` and
//! shifted by `mean`, or a uniform one mapped onto `[lower, upper)`.
//!
//! JSBSim builds both as `aFunc<func_t, 0>`, whose constructor neither
//! loads the element's children nor reads its `name`: any child element
//! is an error, and a `name` is ignored, so the operation has no
//! property and draws only when the function holding it is evaluated.

use super::{FnId, Function, FunctionError, FunctionSet, Op};
use crate::Error;
use crate::random::RandomNumberGenerator;
use crate::xml::{Element, atof_locale_c};

/// The draw of one `<random>` or `<urandom>`.
#[derive(Debug, Clone)]
pub struct RandomDraw {
    /// A normal deviate (`<random>`), else a uniform one (`<urandom>`).
    gaussian: bool,
    /// `stddev`, or `0.5 * (upper - lower)`.
    scale: f64,
    /// `mean`, or `0.5 * (upper + lower)`.
    offset: f64,
    /// The generator a `seed` attribute makes; the executive's otherwise.
    own: Option<RandomNumberGenerator>,
}

impl RandomDraw {
    /// One value: `value * scale + offset`.
    pub(super) fn draw(&self, exec: &RandomNumberGenerator) -> f64 {
        let rng = self.own.as_ref().unwrap_or(exec);
        let value = if self.gaussian {
            rng.normal()
        } else {
            rng.uniform()
        };
        value * self.scale + self.offset
    }
}

/// The value of attribute `name` of `el`, `default` when absent.
fn attribute_or(el: &Element, name: &str, default: f64) -> Result<f64, Error> {
    let text = el.attribute(name);
    if text.is_empty() {
        return Ok(default);
    }
    atof_locale_c(text).map_err(|message| {
        FunctionError::Attribute {
            at: el.location(),
            message,
        }
        .into()
    })
}

/// `makeRandomGenerator`: no `seed` shares the executive's generator, a
/// number seeds one of the function's own through `atoi`, and
/// `time_now` would seed one from the wall clock, which the core does
/// not read.
fn generator(el: &Element) -> Result<Option<RandomNumberGenerator>, Error> {
    match el.attribute("seed") {
        "" => Ok(None),
        "time_now" => Err(Error::Unsupported {
            construct: format!(
                "<{} seed=\"time_now\"> (a clock-seeded generator)",
                el.name()
            ),
            path: el.location().to_string(),
        }),
        seed => Ok(Some(RandomNumberGenerator::new(
            crate::c_numeric::atoi(seed) as u32,
        ))),
    }
}

impl FunctionSet {
    /// `<random>` or `<urandom>` (`op`) at `el`.
    pub(super) fn load_random(&mut self, el: &Element, op: Op) -> Result<FnId, Error> {
        let (gaussian, scale, offset) = if op == Op::Random {
            let mean = attribute_or(el, "mean", 0.0)?;
            let stddev = attribute_or(el, "stddev", 1.0)?;
            (true, stddev, mean)
        } else {
            let lower = attribute_or(el, "lower", -1.0)?;
            let upper = attribute_or(el, "upper", 1.0)?;
            (false, 0.5 * (upper - lower), 0.5 * (upper + lower))
        };
        let own = generator(el)?;
        if !el.children().is_empty() {
            return Err(FunctionError::Arguments {
                element: el.name().to_owned(),
                at: el.location(),
                message: "should have no arguments.".to_owned(),
            }
            .into());
        }
        Ok(self.push_function(Function {
            name: String::new(),
            op,
            params: Vec::new(),
            node: None,
            copy_to: None,
            cached: None,
            at: el.location(),
            random: Some(RandomDraw {
                gaussian,
                scale,
                offset,
                own,
            }),
        }))
    }
}
