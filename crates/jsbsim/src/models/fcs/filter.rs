// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGFilter.{h,cpp}.
//
//! `<lag_filter>`, `<lead_lag_filter>`, `<second_order_filter>` and
//! `<washout_filter>`: Tustin discretisations of first and second order
//! transfer functions.

use super::component::{Base, Tick, model_error, parameter_value};
use super::families::{ComponentAt, Family};
use crate::Error;
use crate::fdm::LoadContext;
use crate::fdm::ModelEvaluator;
use crate::functions::{Evaluator, GetterSource, Parameter};
use crate::xml::Element;

/// Which filter (`FilterType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    /// `C1 / (s + C1)`.
    Lag,
    /// `(C1 s + C2) / (C3 s + C4)`.
    LeadLag,
    /// `(C1 s^2 + C2 s + C3) / (C4 s^2 + C5 s + C6)`.
    SecondOrder,
    /// `s / (s + C1)`.
    Washout,
}

impl FilterType {
    /// How many of `c1`... the coefficients use.
    fn coefficients(self) -> usize {
        match self {
            Self::Lag | Self::Washout => 1,
            Self::LeadLag => 4,
            Self::SecondOrder => 6,
        }
    }
}

/// The difference-equation coefficients `ca`... `ce`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Coefficients {
    /// See the module.
    pub ca: f64,
    /// See the module.
    pub cb: f64,
    /// See the module.
    pub cc: f64,
    /// See the module.
    pub cd: f64,
    /// See the module.
    pub ce: f64,
}

/// `CalculateDynamicFilters` for coefficient values `c` (`c[0]` is `C1`)
/// and step `dt`, in JSBSim's operation order.
#[must_use]
pub fn calculate(kind: FilterType, c: &[f64; 6], dt: f64) -> Coefficients {
    let [c1, c2, c3, c4, c5, c6] = *c;
    let mut k = Coefficients::default();
    match kind {
        FilterType::Lag => {
            let denom = 2.0 + dt * c1;
            k.ca = dt * c1 / denom;
            k.cb = (2.0 - dt * c1) / denom;
        }
        FilterType::LeadLag => {
            let denom = 2.0 * c3 + dt * c4;
            k.ca = (2.0 * c1 + dt * c2) / denom;
            k.cb = (dt * c2 - 2.0 * c1) / denom;
            k.cc = (2.0 * c3 - dt * c4) / denom;
        }
        FilterType::SecondOrder => {
            let denom = 4.0 * c4 + 2.0 * c5 * dt + c6 * dt * dt;
            k.ca = (4.0 * c1 + 2.0 * c2 * dt + c3 * dt * dt) / denom;
            k.cb = (2.0 * c3 * dt * dt - 8.0 * c1) / denom;
            k.cc = (4.0 * c1 - 2.0 * c2 * dt + c3 * dt * dt) / denom;
            k.cd = (2.0 * c6 * dt * dt - 8.0 * c4) / denom;
            k.ce = (4.0 * c4 - 2.0 * c5 * dt + c6 * dt * dt) / denom;
        }
        FilterType::Washout => {
            let denom = 2.0 + dt * c1;
            k.ca = 2.0 / denom;
            k.cb = (2.0 - dt * c1) / denom;
        }
    }
    k
}

/// `FGFilter`.
#[derive(Debug, Clone)]
pub struct Filter {
    /// Which filter.
    pub kind: FilterType,
    /// `<c1>`... `<c6>`, where given.
    pub c: [Option<Parameter>; 6],
    /// Whether a coefficient may change, so the coefficients are
    /// recalculated on every run (`DynamicFilter`).
    pub dynamic: bool,
    /// Whether the next run starts the filter from the last input
    /// (`Initialize`).
    pub initialize: bool,
    /// The difference-equation coefficients.
    pub k: Coefficients,
    /// `PreviousInput1`.
    pub previous_input1: f64,
    /// `PreviousInput2`.
    pub previous_input2: f64,
    /// `PreviousOutput1`.
    pub previous_output1: f64,
    /// `PreviousOutput2`.
    pub previous_output2: f64,
}

/// What one run reads: nothing while initializing, else the input and,
/// for a dynamic filter, the coefficient values.
pub type Sample = Option<(f64, Option<[f64; 6]>)>;

impl Filter {
    /// The values of the coefficients the filter uses, 0 for the others.
    fn values<G: GetterSource>(&self, ev: &mut Evaluator<'_, G>) -> Result<[f64; 6], Error> {
        let mut values = [0.0; 6];
        let used = self.kind.coefficients();
        for (value, c) in values.iter_mut().zip(&self.c).take(used) {
            if let Some(c) = c {
                *value = ev.parameter(c)?;
            }
        }
        Ok(values)
    }

    /// `Run`'s difference equation for input `input`.
    fn output(&self, input: f64) -> f64 {
        let k = &self.k;
        match self.kind {
            FilterType::Lag => (input + self.previous_input1) * k.ca + self.previous_output1 * k.cb,
            FilterType::LeadLag => {
                input * k.ca + self.previous_input1 * k.cb + self.previous_output1 * k.cc
            }
            FilterType::SecondOrder => {
                input * k.ca + self.previous_input1 * k.cb + self.previous_input2 * k.cc
                    - self.previous_output1 * k.cd
                    - self.previous_output2 * k.ce
            }
            FilterType::Washout => {
                input * k.ca - self.previous_input1 * k.ca + self.previous_output1 * k.cb
            }
        }
    }
}

impl Family for Filter {
    type Sample = Sample;

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        _at: ComponentAt,
    ) -> Result<Self, Error> {
        base.check_input_nodes(1, 1, el, ctx)?;
        let kind = match el.name() {
            "lead_lag_filter" => FilterType::LeadLag,
            "second_order_filter" => FilterType::SecondOrder,
            "washout_filter" => FilterType::Washout,
            _ => FilterType::Lag,
        };
        let mut c: [Option<Parameter>; 6] = Default::default();
        let mut dynamic = false;
        for (i, slot) in c.iter_mut().enumerate() {
            if let Some(c_el) = el.find_element(&format!("c{}", i + 1)) {
                let parameter = parameter_value(c_el, ctx)?;
                dynamic |= !is_constant(&parameter, ctx);
                *slot = Some(parameter);
            }
        }
        // JSBSim dereferences a missing coefficient here.
        if let Some(i) = c.iter().take(kind.coefficients()).position(Option::is_none) {
            return Err(model_error(
                el,
                format!("Filter {} has no <c{}>", base.name, i + 1),
            ));
        }
        let mut filter = Self {
            kind,
            c,
            dynamic,
            initialize: true,
            k: Coefficients::default(),
            previous_input1: 0.0,
            previous_input2: 0.0,
            previous_output1: 0.0,
            previous_output2: 0.0,
        };
        let values = {
            let mut ev = Evaluator::new(&*ctx.functions, &mut *ctx.binder.tree);
            filter.values(&mut ev)?
        };
        filter.k = calculate(kind, &values, base.dt);
        base.bind(ctx)?;
        Ok(filter)
    }

    fn sample(&self, base: &Base, ev: &mut ModelEvaluator<'_>) -> Result<Sample, Error> {
        if self.initialize {
            return Ok(None);
        }
        let input = base.read_input(0, ev)?;
        let values = match self.dynamic {
            true => Some(self.values(ev)?),
            false => None,
        };
        Ok(Some((input, values)))
    }

    fn step(&mut self, base: &mut Base, sample: Sample, tick: &Tick) {
        match sample {
            None => {
                // The input member is the last one read, 0 after a reset.
                base.output = base.input;
                self.previous_output2 = base.input;
                self.previous_input2 = base.input;
                self.previous_output1 = base.input;
                self.previous_input1 = base.input;
                self.initialize = false;
            }
            Some((input, values)) => {
                base.input = input;
                if let Some(values) = values {
                    self.k = calculate(self.kind, &values, base.dt);
                }
                base.output = self.output(input);
            }
        }
        self.previous_output2 = self.previous_output1;
        self.previous_output1 = base.output;
        self.previous_input2 = self.previous_input1;
        self.previous_input1 = base.input;
        base.clip(tick);
    }

    fn reset(&mut self, base: &mut Base) {
        base.reset_past_states();
        base.input = 0.0;
        self.initialize = true;
    }
}

/// `IsConstant`: a number, or a property bound now that is neither tied
/// nor writable.
fn is_constant(parameter: &Parameter, ctx: &mut LoadContext<'_>) -> bool {
    match parameter {
        Parameter::Real(_) => true,
        Parameter::Property(p) => p.is_constant(ctx.tree()),
        _ => false,
    }
}
