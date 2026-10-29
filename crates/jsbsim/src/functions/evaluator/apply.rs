// SPDX-License-Identifier: LGPL-2.1-or-later
// FGFunction: Copyright (C) 2004 Jon S. Berndt (jon@jsbsim.org).
// FGTable, FGPropertyValue: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/math/FGFunction.cpp (operation lambdas).
//
//! Applying each operation to its operands, evaluating operands in the
//! order and number JSBSim does.

use std::f64::consts::PI;

use super::{Evaluator, GetterSource};
use crate::Error;
use crate::functions::{Function, FunctionError, Op, Parameter};
use crate::math::{Axis, ColumnVector3, E_X, E_Y, E_Z, Quaternion};
use crate::units::{DEGTORAD, RADTODEG};
use crate::xml::Location;

/// `GetBinary`: |v| < 1e-9 is false, |v| - 1 < 1e-9 is true (so 0.5 is
/// true), anything else is an error.
fn binary(v: f64, at: &Location) -> Result<bool, Error> {
    let v = v.abs();
    if v < 1e-9 {
        Ok(false)
    } else if v - 1.0 < 1e-9 {
        Ok(true)
    } else {
        Err(FunctionError::NotBinary { at: at.clone() }.into())
    }
}

impl<G: GetterSource> Evaluator<'_, G> {
    fn p(&mut self, f: &Function, i: usize) -> Result<f64, Error> {
        match f.params.get(i) {
            Some(param) => self.parameter(param),
            None => Ok(f64::NAN),
        }
    }

    /// Folds the operands' values with `step`, evaluating them in order and
    /// stopping at the first error.
    fn fold(
        &mut self,
        params: &[Parameter],
        init: f64,
        step: impl Fn(f64, f64) -> f64,
    ) -> Result<f64, Error> {
        params
            .iter()
            .try_fold(init, |acc, p| Ok(step(acc, self.parameter(p)?)))
    }

    pub(super) fn apply(&mut self, f: &Function) -> Result<f64, Error> {
        let at = &f.at;
        Ok(match f.op {
            Op::Identity => self.p(f, 0)?,
            Op::Product => self.fold(&f.params, 1.0, |t, v| t * v)?,
            Op::Sum => self.fold(&f.params, 0.0, |t, v| t + v)?,
            Op::Avg => {
                let sum = self.fold(&f.params, 0.0, |t, v| t + v)?;
                sum / f.params.len() as f64
            }
            Op::Difference => {
                let first = self.p(f, 0)?;
                let rest = f.params.get(1..).unwrap_or_default();
                self.fold(rest, first, |t, v| t - v)?
            }
            Op::Min => self.fold(&f.params, f64::INFINITY, |m, x| if x < m { x } else { m })?,
            Op::Max => self.fold(
                &f.params,
                f64::NEG_INFINITY,
                |m, x| {
                    if x > m { x } else { m }
                },
            )?,
            Op::And | Op::Or => self.logical(f)?,
            Op::Not => f64::from(u8::from(!binary(self.p(f, 0)?, at)?)),
            Op::IfThen => {
                if binary(self.p(f, 0)?, at)? {
                    self.p(f, 1)?
                } else {
                    self.p(f, 2)?
                }
            }
            Op::Switch => self.switch(f)?,
            Op::Interpolate1d => self.interpolate1d(f)?,
            Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::Eq | Op::Nq => self.compare(f)?,
            Op::RotationAlphaLocal | Op::RotationBetaLocal | Op::RotationGammaLocal => {
                self.rotation_local(f)?
            }
            Op::RotationBfToWf | Op::RotationWfToBf => self.rotation_frame(f)?,
            _ => self.arithmetic(f)?,
        })
    }

    fn arithmetic(&mut self, f: &Function) -> Result<f64, Error> {
        let at = &f.at;
        Ok(match f.op {
            Op::Quotient => {
                let y = self.p(f, 1)?;
                if y != 0.0 {
                    self.p(f, 0)? / y
                } else {
                    f64::INFINITY
                }
            }
            Op::Fmod => {
                let y = self.p(f, 1)?;
                if y != 0.0 {
                    libm::fmod(self.p(f, 0)?, y)
                } else {
                    f64::INFINITY
                }
            }
            Op::RoundMultiple if f.params.len() == 1 => libm::round(self.p(f, 0)?),
            Op::RoundMultiple => {
                let multiple = self.p(f, 1)?;
                libm::round(self.p(f, 0)? / multiple) * multiple
            }
            Op::Pow => {
                let x = self.p(f, 0)?;
                libm::pow(x, self.p(f, 1)?)
            }
            Op::Atan2 => {
                let y = self.p(f, 0)?;
                libm::atan2(y, self.p(f, 1)?)
            }
            Op::Mod => {
                let a = self.p(f, 0)? as i32;
                let b = self.p(f, 1)? as i32;
                f64::from(
                    a.checked_rem(b)
                        .ok_or_else(|| FunctionError::ModByZero { at: at.clone() })?,
                )
            }
            _ => unary(f.op, self.p(f, 0)?),
        })
    }
}

fn unary(op: Op, x: f64) -> f64 {
    match op {
        Op::ToRadians => x * PI / 180.,
        Op::ToDegrees => x * 180. / PI,
        Op::Sqrt => {
            if x >= 0.0 {
                x.sqrt()
            } else {
                f64::NEG_INFINITY
            }
        }
        Op::Log2 => {
            if x > 0.0 {
                libm::log10(x) * (1.0 / libm::log10(2.0))
            } else {
                f64::NEG_INFINITY
            }
        }
        Op::Ln => {
            if x > 0.0 {
                libm::log(x)
            } else {
                f64::NEG_INFINITY
            }
        }
        Op::Log10 => {
            if x > 0.0 {
                libm::log10(x)
            } else {
                f64::NEG_INFINITY
            }
        }
        Op::Sign => {
            if x < 0.0 {
                -1.0
            } else {
                1.0
            }
        }
        Op::Exp => libm::exp(x),
        Op::Abs => x.abs(),
        Op::Sin => libm::sin(x),
        Op::Cos => libm::cos(x),
        Op::Tan => libm::tan(x),
        Op::Asin => libm::asin(x),
        Op::Acos => libm::acos(x),
        Op::Atan => libm::atan(x),
        Op::Floor => libm::floor(x),
        Op::Ceil => libm::ceil(x),
        Op::Fraction => libm::modf(x).0,
        Op::Integer => libm::modf(x).1,
        _ => f64::NAN,
    }
}

impl<G: GetterSource> Evaluator<'_, G> {
    fn logical(&mut self, f: &Function) -> Result<f64, Error> {
        let short = f.op == Op::Or;
        for param in &f.params {
            if binary(self.parameter(param)?, &f.at)? == short {
                return Ok(f64::from(u8::from(short)));
            }
        }
        Ok(f64::from(u8::from(!short)))
    }

    fn compare(&mut self, f: &Function) -> Result<f64, Error> {
        let a = self.p(f, 0)?;
        let b = self.p(f, 1)?;
        let pass = match f.op {
            Op::Lt => a < b,
            Op::Le => a <= b,
            Op::Gt => a > b,
            Op::Ge => a >= b,
            Op::Eq => a == b,
            _ => a != b,
        };
        Ok(f64::from(u8::from(pass)))
    }

    fn switch(&mut self, f: &Function) -> Result<f64, Error> {
        let temp = self.p(f, 0)?;
        if temp < 0.0 {
            return Err(FunctionError::Index {
                at: f.at.clone(),
                message: format!("The switch function index ({temp}) is negative."),
            }
            .into());
        }
        let values = f.params.split_first().map_or(&[][..], |(_, values)| values);
        let i = (temp + 0.5) as usize;
        if let Some(value) = values.get(i) {
            return self.parameter(value);
        }
        // Load-time arity gives at least one value, so the bound is exact.
        Err(FunctionError::Index {
            at: f.at.clone(),
            message: format!(
                "The switch function index ({temp}) selected a value above the range of supplied values[0:{}] - not enough values were supplied.",
                values.len().saturating_sub(1)
            ),
        }
        .into())
    }

    /// Bisection over `x, x1, y1, x2, y2, ...`, evaluating each operand
    /// at most once.
    fn interpolate1d(&mut self, f: &Function) -> Result<f64, Error> {
        let n = f.params.len();
        let x = self.p(f, 0)?;
        let mut xmin = self.p(f, 1)?;
        let mut ymin = self.p(f, 2)?;
        if x <= xmin {
            return Ok(ymin);
        }
        // Load-time arity makes `n` odd and at least 5; anything shorter has
        // no last breakpoint and reads as NaN, like a missing operand.
        let Some(segments) = n.checked_sub(3) else {
            return Ok(f64::NAN);
        };
        let mut xmax = self.p(f, segments + 1)?;
        let mut ymax = self.p(f, segments + 2)?;
        if x >= xmax {
            return Ok(ymax);
        }
        let (mut nmin, mut nmax) = (0, segments / 2);
        while nmax - nmin > 1 {
            let m = (nmax - nmin) / 2 + nmin;
            let xm = self.p(f, 2 * m + 1)?;
            let ym = self.p(f, 2 * m + 2)?;
            if x < xm {
                (xmax, ymax, nmax) = (xm, ym, m);
            } else if x > xm {
                (xmin, ymin, nmin) = (xm, ym, m);
            } else {
                return Ok(ym);
            }
        }
        Ok(ymin + (x - xmin) * (ymax - ymin) / (xmax - xmin))
    }

    fn rotation_local(&mut self, f: &Function) -> Result<f64, Error> {
        let alpha = self.p(f, 0)? * DEGTORAD;
        let beta = self.p(f, 1)? * DEGTORAD;
        let gamma = if f.op == Op::RotationGammaLocal {
            self.p(f, 2)? * DEGTORAD
        } else {
            0.0
        };
        let phi = self.p(f, 3)? * DEGTORAD;
        let theta = self.p(f, 4)? * DEGTORAD;
        let psi = self.p(f, 5)? * DEGTORAD;
        let t = Quaternion::from_euler(phi, theta, psi).t();
        Ok(match f.op {
            Op::RotationAlphaLocal | Op::RotationBetaLocal => {
                let (sin_alpha, cos_alpha) = libm::sincos(alpha);
                let (sin_beta, cos_beta) = libm::sincos(beta);
                let wind =
                    t * ColumnVector3::new(cos_alpha * cos_beta, sin_beta, sin_alpha * cos_beta);
                local_alpha_beta(f.op, wind)
            }
            _ => local_gamma(&t, alpha, beta, gamma),
        })
    }

    fn rotation_frame(&mut self, f: &Function) -> Result<f64, Error> {
        let rx = self.p(f, 0)?;
        let ry = self.p(f, 1)?;
        let rz = self.p(f, 2)?;
        let alpha = self.p(f, 3)? * DEGTORAD;
        let beta = self.p(f, 4)? * DEGTORAD;
        let gamma = self.p(f, 5)? * DEGTORAD;
        let idx = self.p(f, 6)? as i32;
        let axis = usize::try_from(idx).ok().and_then(Axis::from_index1);
        let Some(axis) = axis else {
            return Err(FunctionError::Index {
                at: f.at.clone(),
                message: "The index must be one of the integer value 1, 2 or 3.".to_owned(),
            }
            .into());
        };
        let qa = Quaternion::from_axis_index(E_Y, -alpha);
        let qb = Quaternion::from_axis_index(E_Z, beta);
        let qc = Quaternion::from_axis_index(E_X, -gamma);
        let mut t = (&(&qa * &qb) * &qc).t();
        if f.op == Op::RotationWfToBf {
            t.transpose();
        }
        Ok((t * ColumnVector3::new(rx, ry, rz))[axis])
    }
}

fn local_alpha_beta(op: Op, wind: ColumnVector3) -> f64 {
    let near_pole = ((wind[E_Y]).abs() - 1.0).abs() < 1e-9;
    if op == Op::RotationAlphaLocal {
        return if near_pole {
            0.0
        } else {
            libm::atan2(wind[E_Z], wind[E_X]) * RADTODEG
        };
    }
    if near_pole {
        return if wind[E_Y] > 0.0 { 0.5 * PI } else { -0.5 * PI };
    }
    let alpha_local = libm::atan2(wind[E_Z], wind[E_X]);
    let (sina, cosa) = libm::sincos(alpha_local);
    let cosb = if cosa.abs() > sina.abs() {
        wind[E_X] / cosa
    } else {
        wind[E_Z] / sina
    };
    libm::atan2(wind[E_Y], cosb) * RADTODEG
}

fn local_gamma(t: &crate::math::Matrix33, alpha: f64, beta: f64, gamma: f64) -> f64 {
    let (sin_alpha, cos_alpha) = libm::sincos(alpha);
    let (sin_beta, cos_beta) = libm::sincos(beta);
    let (sin_gamma, cos_gamma) = libm::sincos(gamma);
    let x = *t * ColumnVector3::new(cos_alpha * cos_beta, sin_beta, sin_alpha * cos_beta);
    let y = *t
        * ColumnVector3::new(
            -sin_alpha * sin_gamma - sin_beta * cos_alpha * cos_gamma,
            cos_beta * cos_gamma,
            -sin_alpha * sin_beta * cos_gamma + sin_gamma * cos_alpha,
        );
    let (cosacosb, sinb, sinacosb) = (x[E_X], x[E_Y], x[E_Z]);
    let (sinc, cosc);
    if sinb.abs() < 1e-9 {
        cosc = y[E_Y];
        sinc = if cosacosb.abs() > sinacosb.abs() {
            y[E_Z] / cosacosb
        } else {
            -y[E_X] / sinacosb
        };
    } else if (sinb.abs() - 1.0).abs() < 1e-9 {
        sinc = y[E_Z];
        cosc = -y[E_X];
    } else {
        sinc = cosacosb * y[E_Z] - sinacosb * y[E_X];
        cosc = (-sinacosb * y[E_Z] - cosacosb * y[E_X]) / sinb;
    }
    libm::atan2(sinc, cosc) * RADTODEG
}
