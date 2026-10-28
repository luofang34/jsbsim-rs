// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2013 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGDistributor.{h,cpp}.
//
//! `<distributor>`: cases whose test passes (every one, or only the
//! first, by `type`) set their properties to values; a case without a test
//! always does.
//!
//! The cases run in order and each value is written as it is read, so a
//! later case reads what an earlier one wrote in the same run. That does
//! not fit [`Family`]'s read-then-update split, so the run is the
//! executive's: [`Distributor::fires`] and [`Distributor::pair`] evaluate
//! one step each and the executive writes the value between them. The
//! family's own `sample` and `step` do nothing, and a distributor has no
//! outputs: JSBSim's `Run` neither clips nor calls `SetOutput`.

#[cfg(test)]
mod tests;

use super::Fcs;
use super::Kind;
use super::component::{Base, Tick, model_error};
use super::families::{ComponentAt, Family};
use crate::Error;
use crate::fdm::LoadContext;
use crate::functions::{Condition, Evaluator, Parameter, PropertyValue};
use crate::properties::NodeId;
use crate::xml::Element;

/// A `<property value="...">` of a case.
#[derive(Debug, Clone)]
pub struct PropValPair {
    /// The property set; a sign is ignored, as JSBSim's release build
    /// ignores it.
    pub property: PropertyValue,
    /// The value: a number or a property.
    pub value: Parameter,
}

/// A `<case>`.
#[derive(Debug, Clone)]
pub struct Case {
    /// The `<test>`, if any.
    pub test: Option<Condition>,
    /// The properties set, in order.
    pub pairs: Vec<PropValPair>,
}

/// `FGDistributor`.
#[derive(Debug, Clone)]
pub struct Distributor {
    /// `type="exclusive"`: only the first passing tested case sets.
    pub exclusive: bool,
    /// The cases, in order.
    pub cases: Vec<Case>,
}

/// The distributor at `at`, if the component there is one.
#[must_use]
pub fn at(fcs: &Fcs, at: ComponentAt) -> Option<&Distributor> {
    match &fcs.channels.get(at.channel)?.components.get(at.index)?.kind {
        Kind::Distributor(d) => Some(d),
        _ => None,
    }
}

fn load_case(el: &Element, ctx: &mut LoadContext<'_>) -> Result<Case, Error> {
    let test = match el.find_element("test") {
        Some(test_el) => Some(Condition::load(test_el, ctx.tree())?),
        None => None,
    };
    let mut pairs = Vec::new();
    for pair_el in el.find_elements("property") {
        let at = pair_el.location();
        let property = PropertyValue::new(pair_el.data_line(0), ctx.tree(), at.clone())?;
        let value = Parameter::number_or_property(pair_el.attribute("value"), ctx.tree(), at)?;
        pairs.push(PropValPair { property, value });
    }
    Ok(Case { test, pairs })
}

impl Distributor {
    /// Whether case `case` sets its properties this run, after
    /// `completed` (a tested case has set already): `(sets, completes)`.
    /// The test is evaluated even when an exclusive distributor has
    /// completed, as JSBSim evaluates it first.
    pub fn fires(
        &self,
        case: usize,
        completed: bool,
        ev: &mut Evaluator<'_>,
    ) -> Result<(bool, bool), Error> {
        let Some(case) = self.cases.get(case) else {
            return Ok((false, false));
        };
        match &case.test {
            Some(test) => {
                let sets = test.evaluate(ev)? && !(self.exclusive && completed);
                Ok((sets, sets))
            }
            None => Ok((true, false)),
        }
    }

    /// The node and value of pair `pair` of case `case`
    /// (`SetPropToValue`'s reads): the value first, then the property,
    /// bound now if needed. Either failing is JSBSim's "not known" error.
    pub fn pair(
        &self,
        case: usize,
        pair: usize,
        ev: &mut Evaluator<'_>,
    ) -> Result<Option<(NodeId, f64)>, Error> {
        let Some(p) = self.cases.get(case).and_then(|c| c.pairs.get(pair)) else {
            return Ok(None);
        };
        let not_known = |_| Error::Model {
            message: format!(
                "{} in distributor component is not known",
                p.property.name()
            ),
        };
        let value = ev.parameter(&p.value).map_err(not_known)?;
        let node = p.property.node(ev.tree()).map_err(not_known)?;
        Ok(Some((node, value)))
    }
}

impl Family for Distributor {
    type Sample = ();

    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        _at: ComponentAt,
    ) -> Result<Self, Error> {
        // Bound first, so a case can read the component's own property.
        base.bind(ctx)?;
        // Run neither clips nor sets the outputs.
        base.outputs.clear();
        base.clip = None;
        let exclusive = match el.attribute("type") {
            "inclusive" => false,
            "exclusive" => true,
            other => {
                return Err(model_error(
                    el,
                    format!(
                        "Distributor type should be \"inclusive\" or \"exclusive\" but got \"{other}\" instead."
                    ),
                ));
            }
        };
        let mut cases = Vec::new();
        for case_el in el.find_elements("case") {
            cases.push(load_case(case_el, ctx)?);
        }
        Ok(Self { exclusive, cases })
    }

    fn sample(&self, _base: &Base, _ev: &mut Evaluator<'_>) -> Result<(), Error> {
        Ok(())
    }

    fn step(&mut self, _base: &mut Base, _sample: (), _tick: &Tick) {}
}

/// The number of cases of the distributor at `at`; `None` when the
/// component there is not a distributor.
#[must_use]
pub fn cases(fcs: &Fcs, at: ComponentAt) -> Option<usize> {
    self::at(fcs, at).map(|d| d.cases.len())
}

/// The number of pairs of case `case` of the distributor at `at`.
#[must_use]
pub fn pairs(fcs: &Fcs, at: ComponentAt, case: usize) -> usize {
    self::at(fcs, at)
        .and_then(|d| d.cases.get(case))
        .map_or(0, |c| c.pairs.len())
}
