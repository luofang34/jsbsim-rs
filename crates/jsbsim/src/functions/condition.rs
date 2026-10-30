// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2003 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/math/FGCondition.{h,cpp}.
//
//! `<test>`-style conditions: comparisons of a property with a number or
//! property, combined with AND / OR.

use super::{Evaluator, FunctionError, GetterSource, Parameter, PropertyValue};
use crate::Error;
use crate::c_numeric::is_c_space;
use crate::properties::PropertyTree;
use crate::xml::Element;

/// A comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    /// `==`, `EQ`, `eq`
    Eq,
    /// `!=`, `NE`, `ne`
    Ne,
    /// `>`, `GT`, `gt`
    Gt,
    /// `>=`, `GE`, `ge`
    Ge,
    /// `<`, `LT`, `lt`
    Lt,
    /// `<=`, `LE`, `le`
    Le,
}

impl Comparison {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "==" | "EQ" | "eq" => Self::Eq,
            "!=" | "NE" | "ne" => Self::Ne,
            ">" | "GT" | "gt" => Self::Gt,
            ">=" | "GE" | "ge" => Self::Ge,
            "<" | "LT" | "lt" => Self::Lt,
            "<=" | "LE" | "le" => Self::Le,
            _ => return None,
        })
    }
}

/// `FGCondition`.
#[derive(Debug, Clone)]
pub enum Condition {
    /// Sub-conditions combined with AND (`all`) or OR.
    Group {
        /// AND when true, OR when false.
        all: bool,
        /// The sub-conditions.
        conditions: Vec<Self>,
    },
    /// `property comparison value`.
    Test {
        /// Left operand.
        left: PropertyValue,
        /// Operator.
        comparison: Comparison,
        /// Right operand: a number or a property.
        right: Parameter,
    },
}

impl Condition {
    /// `FGCondition(element, pm)`: each data line is a test, each child
    /// element (which must have this element's name) a nested group.
    pub fn load(el: &Element, tree: &PropertyTree) -> Result<Self, Error> {
        let fail = |message: String| -> Error {
            FunctionError::Condition {
                at: el.location(),
                message,
            }
            .into()
        };
        let all = match el.attribute("logic") {
            "" | "AND" => true,
            "OR" => false,
            other => return Err(fail(format!("unrecognized LOGIC token:'{other}'"))),
        };
        let mut conditions = Vec::new();
        for line in el.data_lines() {
            conditions.push(Self::load_test(line, el, tree)?);
        }
        for child in el.children() {
            if child.name() != el.name() {
                return Err(fail(format!(
                    "unrecognized TAG:'{}' in the condition statement.",
                    child.name()
                )));
            }
            conditions.push(Self::load(child, tree)?);
        }
        if conditions.is_empty() {
            return Err(fail("Empty conditional".to_owned()));
        }
        Ok(Self::Group { all, conditions })
    }

    /// `FGCondition(test, pm, el)`: `property op value`, split at single
    /// spaces as JSBSim's `split(test, ' ')` does.
    pub fn load_test(test: &str, el: &Element, tree: &PropertyTree) -> Result<Self, Error> {
        let trim = |s: &str| {
            s.trim_matches(|c: char| c.is_ascii() && is_c_space(c as u8))
                .to_owned()
        };
        let parts: Vec<String> = trim(test)
            .split(' ')
            .map(trim)
            .filter(|p| !p.is_empty())
            .collect();
        let fail = |message: String| -> Error {
            FunctionError::Condition {
                at: el.location(),
                message,
            }
            .into()
        };
        let [left, op, right] = parts.as_slice() else {
            return Err(fail(format!(
                "  Conditional test is invalid: \"{test}\" has {} elements in the test condition.",
                parts.len()
            )));
        };
        let left = PropertyValue::new(left, tree, el.location())?;
        let right = Parameter::number_or_property(right, tree, el.location())?;
        let comparison = Comparison::parse(op).ok_or_else(|| {
            fail(format!(
                "Comparison operator: \"{op}\" does not exist.  Please check the conditional."
            ))
        })?;
        Ok(Self::Test {
            left,
            comparison,
            right,
        })
    }

    /// `Evaluate()`: every sub-condition is evaluated, without
    /// short-circuit, as in JSBSim.
    pub fn evaluate<G: GetterSource>(&self, ev: &mut Evaluator<'_, G>) -> Result<bool, Error> {
        match self {
            Self::Group { all, conditions } => {
                let mut pass = *all;
                for condition in conditions {
                    let result = condition.evaluate(ev)?;
                    if *all && !result {
                        pass = false;
                    } else if !*all && result {
                        pass = true;
                    }
                }
                Ok(pass)
            }
            Self::Test {
                left,
                comparison,
                right,
            } => {
                let a = ev.property(left)?;
                let b = ev.parameter(right)?;
                Ok(match comparison {
                    Comparison::Eq => a == b,
                    Comparison::Ne => a != b,
                    Comparison::Gt => a > b,
                    Comparison::Ge => a >= b,
                    Comparison::Lt => a < b,
                    Comparison::Le => a <= b,
                })
            }
        }
    }
}

impl crate::snapshot::Visit for Condition {
    fn visit<V: crate::snapshot::Visitor>(
        &mut self,
        v: &mut V,
    ) -> Result<(), crate::snapshot::SnapshotError> {
        match self {
            Self::Group { conditions, .. } => crate::snapshot::Visit::visit(conditions, v),
            Self::Test { left, right, .. } => {
                crate::snapshot::Visit::visit(left, v)?;
                crate::snapshot::Visit::visit(right, v)
            }
        }
    }
}
