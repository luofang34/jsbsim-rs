// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2005 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGSwitch.{h,cpp}.
//
//! `<switch>`: the value of the first test whose condition passes, else
//! the default.

use super::component::{Base, Tick, lenient};
use super::families::{ComponentAt, Family};
use crate::Error;
use crate::fdm::LoadContext;
use crate::fdm::ModelEvaluator;
use crate::functions::{Condition, FunctionError, Parameter};
use crate::xml::Element;

/// One `<test>`, or the `<default>` when it has no condition.
#[derive(Debug, Clone)]
pub struct Test {
    /// The condition; `None` for the default.
    pub condition: Option<Condition>,
    /// `value`: a number or a property.
    pub value: Parameter,
}

/// `FGSwitch`.
#[derive(Debug, Clone)]
pub struct Switch {
    /// The default first, if any, then the tests in order.
    pub tests: Vec<Test>,
    /// Whether the first run, which checks every test, has happened.
    pub initialized: bool,
}

/// `Test::setTestValue`: the value, or `None` (logged) when the element
/// has no `value`.
fn test_value(
    el: &Element,
    name: &str,
    what: &str,
    ctx: &mut LoadContext<'_>,
) -> Result<Option<Parameter>, Error> {
    let value = el.attribute("value");
    if value.is_empty() {
        let message =
            format!("No VALUE supplied for switch component: {name}\n    {what} IGNORED.");
        lenient(el, ctx, &message)?;
        return Ok(None);
    }
    Ok(Some(Parameter::number_or_property(
        value,
        ctx.tree(),
        el.location(),
    )?))
}

/// One `<test>`; a malformed condition is logged and the test ignored, as
/// JSBSim's `catch (BaseException&)` does.
fn load_test(el: &Element, name: &str, ctx: &mut LoadContext<'_>) -> Result<Option<Test>, Error> {
    let condition = match Condition::load(el, ctx.tree()) {
        Ok(condition) => condition,
        Err(Error::Function(e @ FunctionError::Condition { .. })) => {
            lenient(el, ctx, &format!("{e}\n    Test IGNORED."))?;
            return Ok(None);
        }
        Err(e) => return Err(e),
    };
    Ok(test_value(el, name, "Test", ctx)?.map(|value| Test {
        condition: Some(condition),
        value,
    }))
}

impl Family for Switch {
    /// The output chosen.
    type Sample = f64;

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        _at: ComponentAt,
    ) -> Result<Self, Error> {
        // Bound first, so a test can read the switch's own output (a
        // sample-and-hold).
        base.bind(ctx)?;
        let mut tests = Vec::new();
        if let Some(default) = el.find_element("default")
            && let Some(value) = test_value(default, &base.name, "Default value", ctx)?
        {
            // JSBSim's prefill of the delay buffer with a numeric default
            // never runs: it tests the value for FGRealValue, but values
            // are FGParameterValue wrappers.
            tests.push(Test {
                condition: None,
                value,
            });
        }
        for test_el in el.find_elements("test") {
            if let Some(test) = load_test(test_el, &base.name, ctx)? {
                tests.push(test);
            }
        }
        Ok(Self {
            tests,
            initialized: false,
        })
    }

    fn sample(&self, _base: &Base, ev: &mut ModelEvaluator<'_>) -> Result<f64, Error> {
        // `VerifyProperties`: every condition and value is evaluated on the
        // first run, so a missing property fails then.
        if !self.initialized {
            for test in &self.tests {
                if let Some(condition) = &test.condition {
                    condition.evaluate(ev)?;
                }
                ev.parameter(&test.value)?;
            }
        }
        let mut default_output = 0.0;
        for test in &self.tests {
            let pass = match &test.condition {
                None => {
                    default_output = ev.parameter(&test.value)?;
                    false
                }
                Some(condition) => condition.evaluate(ev)?,
            };
            if pass {
                return ev.parameter(&test.value);
            }
        }
        Ok(default_output)
    }

    fn step(&mut self, base: &mut Base, output: f64, tick: &Tick) {
        self.initialized = true;
        base.output = output;
        if !base.delay.is_empty() {
            base.delay(tick);
        }
        base.clip(tick);
    }
}
