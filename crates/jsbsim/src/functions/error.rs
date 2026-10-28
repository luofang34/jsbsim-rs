// SPDX-License-Identifier: LGPL-2.1-or-later
// FGFunction: Copyright (C) 2004 Jon S. Berndt (jon@jsbsim.org).
// FGTemplateFunc: Copyright (C) 2018 B. Coconnier (bcoconni@users.sf.net).
// Derived from JSBSim src/math/FGFunction.cpp, FGTable.cpp, FGCondition.cpp
// and FGPropertyValue.cpp (the exceptions they throw).
//
//! Errors of tables, functions and conditions, at load time and at run time.

use thiserror::Error;

use crate::xml::Location;

/// Error raised while loading or evaluating a table, function or condition.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FunctionError {
    /// An operation has the wrong number of arguments.
    #[error("{at}: <{element}> {message}")]
    Arguments {
        /// Operation element.
        element: String,
        /// Where it is.
        at: Location,
        /// JSBSim's message, e.g. "should have at least 2 argument(s).".
        message: String,
    },
    /// `#` used without a numeric prefix to substitute.
    #[error("{at}: Illegal use of the special character '#'")]
    IllegalHash {
        /// Where it is.
        at: Location,
    },
    /// A `type="internal"` table inside a function.
    #[error("{at}: An internal table cannot be nested within a function.")]
    InternalTableInFunction {
        /// Where the function is.
        at: Location,
    },
    /// The output property of a function or table is already bound.
    #[error("{at}: Property {property} has already been successfully bound (late).")]
    AlreadyBound {
        /// Property path.
        property: String,
        /// Where the function or table is.
        at: Location,
    },
    /// A malformed table.
    #[error("{at}: FGTable: {message}")]
    Table {
        /// Where the table is.
        at: Location,
        /// What is wrong.
        message: String,
    },
    /// A malformed condition.
    #[error("{at}: FGCondition: {message}")]
    Condition {
        /// Where the condition is.
        at: Location,
        /// What is wrong.
        message: String,
    },
    /// A late-bound property was still undefined when first read.
    #[error("{at}: Property {property} does not exist")]
    MissingProperty {
        /// Property path.
        property: String,
        /// Where it was referenced.
        at: Location,
    },
    /// A logical operand is neither 0 nor 1 (within 1e-9).
    #[error("{at}: Malformed conditional check in function definition.")]
    NotBinary {
        /// Where the operation is.
        at: Location,
    },
    /// An index operand selects nothing.
    #[error("{at}: {message}")]
    Index {
        /// Where the operation is.
        at: Location,
        /// JSBSim's message.
        message: String,
    },
    /// `<mod>` with a divisor that truncates to 0, which crashes JSBSim.
    #[error("{at}: <mod> divisor truncates to zero")]
    ModByZero {
        /// Where the operation is.
        at: Location,
    },
    /// Evaluating a property reads the property itself, which recurses
    /// without end in JSBSim.
    #[error("evaluation of {property} recursed into itself")]
    Recursion {
        /// Property being evaluated.
        property: String,
    },
    /// A template parameter read with no property to stand for.
    #[error("{at}: template parameter '#' read outside a template call")]
    UnboundTemplateParameter {
        /// Where it is.
        at: Location,
    },
}
