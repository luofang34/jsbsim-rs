// SPDX-License-Identifier: LGPL-2.1-or-later
// FGFunction: Copyright (C) 2004 Jon S. Berndt (jon@jsbsim.org).
// FGTable, FGPropertyValue: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/math/FGFunction.cpp (operations).
//
//! The operations of `<function>`: their names, argument rules and
//! evaluation. Operands are evaluated in the order and number JSBSim
//! evaluates them, since reading a bound property can have effects.

/// Argument-count parity rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    /// Any count.
    Either,
    /// An odd count.
    Odd,
}

macro_rules! operations {
    ($($variant:ident => $name:literal, $min:expr, $max:expr, $parity:ident, $varargs:expr;)*) => {
        /// A function operation.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Op {
            /// The `<function>` element itself: its single operand.
            Identity,
            $(
                #[doc = concat!("`<", $name, ">`")]
                $variant,
            )*
        }

        /// Names of every operation element, in `FGFunction::Load` order.
        pub const OPERATIONS: &[&str] = &[$($name),*];

        impl Op {
            /// The operation named `name`.
            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(Self::$variant),)*
                    _ => None,
                }
            }

            /// Element name.
            #[must_use]
            pub fn name(self) -> &'static str {
                match self {
                    Self::Identity => "function",
                    $(Self::$variant => $name,)*
                }
            }

            /// `(min, max, parity)` argument counts.
            #[must_use]
            pub fn arity(self) -> (usize, usize, Parity) {
                match self {
                    Self::Identity => (1, 1, Parity::Either),
                    $(Self::$variant => ($min, $max, Parity::$parity),)*
                }
            }

            /// Whether a single argument makes the operation a no-op that
            /// is replaced by its argument (`VarArgsFn`).
            #[must_use]
            pub fn collapses_single_argument(self) -> bool {
                match self {
                    Self::Identity => false,
                    $(Self::$variant => $varargs,)*
                }
            }
        }
    };
}

const MANY: usize = 9999;

operations! {
    Product => "product", 2, MANY, Either, true;
    Sum => "sum", 2, MANY, Either, true;
    Avg => "avg", 2, MANY, Either, true;
    Difference => "difference", 2, MANY, Either, true;
    Min => "min", 2, MANY, Either, true;
    Max => "max", 2, MANY, Either, true;
    And => "and", 2, MANY, Either, false;
    Or => "or", 2, MANY, Either, false;
    Quotient => "quotient", 2, 2, Either, false;
    Pow => "pow", 2, 2, Either, false;
    ToRadians => "toradians", 1, 1, Either, false;
    ToDegrees => "todegrees", 1, 1, Either, false;
    Sqrt => "sqrt", 1, 1, Either, false;
    Log2 => "log2", 1, 1, Either, false;
    Ln => "ln", 1, 1, Either, false;
    Log10 => "log10", 1, 1, Either, false;
    Sign => "sign", 1, 1, Either, false;
    Exp => "exp", 1, 1, Either, false;
    Abs => "abs", 1, 1, Either, false;
    Sin => "sin", 1, 1, Either, false;
    Cos => "cos", 1, 1, Either, false;
    Tan => "tan", 1, 1, Either, false;
    Asin => "asin", 1, 1, Either, false;
    Acos => "acos", 1, 1, Either, false;
    Atan => "atan", 1, 1, Either, false;
    Floor => "floor", 1, 1, Either, false;
    Ceil => "ceil", 1, 1, Either, false;
    Fmod => "fmod", 2, 2, Either, false;
    RoundMultiple => "roundmultiple", 1, 2, Either, false;
    Atan2 => "atan2", 2, 2, Either, false;
    Mod => "mod", 2, 2, Either, false;
    Fraction => "fraction", 1, 1, Either, false;
    Integer => "integer", 1, 1, Either, false;
    Lt => "lt", 2, 2, Either, false;
    Le => "le", 2, 2, Either, false;
    Gt => "gt", 2, 2, Either, false;
    Ge => "ge", 2, 2, Either, false;
    Eq => "eq", 2, 2, Either, false;
    Nq => "nq", 2, 2, Either, false;
    Not => "not", 1, 1, Either, false;
    IfThen => "ifthen", 3, 3, Either, false;
    Random => "random", 0, 0, Either, false;
    URandom => "urandom", 0, 0, Either, false;
    Switch => "switch", 2, MANY, Either, false;
    Interpolate1d => "interpolate1d", 5, MANY, Odd, false;
    RotationAlphaLocal => "rotation_alpha_local", 6, 6, Either, false;
    RotationBetaLocal => "rotation_beta_local", 6, 6, Either, false;
    RotationGammaLocal => "rotation_gamma_local", 6, 6, Either, false;
    RotationBfToWf => "rotation_bf_to_wf", 7, 7, Either, false;
    RotationWfToBf => "rotation_wf_to_bf", 7, 7, Either, false;
}

/// Operations this port rejects with [`Error::Unsupported`](crate::Error::Unsupported):
/// JSBSim's random generator is not mirrored yet.
pub const UNSUPPORTED: &[Op] = &[Op::Random, Op::URandom];
