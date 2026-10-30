// SPDX-License-Identifier: LGPL-2.1-or-later
//! Internals that `tools/oracle` reaches. **Not part of the supported API:**
//! anything here may change or vanish in any release.
//!
//! The oracle's probes mirror JSBSim's C++ classes one for one and compare
//! each against its C++ twin, so they need parts of the port that a flight
//! simulation front end never does. Each submodule re-exports, under the
//! crate-internal module's name, exactly the items the oracle uses. A few
//! more hooks are hidden in place rather than re-exported here: the
//! `Fdm::models` field, `Fdm::tree` and the other `#[doc(hidden)]` methods
//! of [`Fdm`](crate::Fdm), `trim::Trim`, and
//! `math::UNCACHED_DERIVED_TARGET`.

#![doc(hidden)]

/// C's numeric parsing and printf formatting.
pub mod c_numeric {
    pub use crate::c_numeric::{atoi, format_g};
}

/// The elementary functions.
pub mod elementary {
    pub use crate::elementary::FORWARDS_TO_LIBM;
}

/// Functions, tables and their evaluation.
pub mod functions {
    pub use crate::functions::{Evaluator, FnId, FunctionSet, Table};
}

/// Initial conditions.
pub mod initial_condition {
    pub use crate::initial_condition::SpeedSet;
}

/// Linearization about the trimmed state.
pub mod linearization {
    pub use crate::linearization::Linearization;
}

/// The models.
pub mod models {
    /// `FGAtmosphere` and its models.
    pub mod atmosphere {
        pub use crate::models::atmosphere::{AtmosphereModel, PressureUnit, TemperatureUnit};
    }

    /// `FGAuxiliary`.
    pub mod auxiliary {
        pub use crate::models::auxiliary::{
            Inputs, mach_from_impact_pressure, pitot_total_pressure,
        };
    }

    /// `FGPropulsion`.
    pub mod propulsion {
        /// The engine and thruster families.
        pub mod families {
            pub use crate::models::propulsion::families::{
                REGISTERED_ENGINES, REGISTERED_THRUSTERS,
            };
        }

        /// `FGTank`.
        pub mod tank {
            pub use crate::models::propulsion::tank::{GrainType, TankType};
        }
    }
}

/// The property tree.
pub mod properties {
    pub use crate::properties::{
        Attribute, NodeId, PathError, PropertyTree, Value, ValueType, mk_property_name,
    };
}

/// The random number generators.
pub mod random {
    pub use crate::random::{RandomNumberGenerator, SharedGenerator};
}

/// Unit conversion constants and functions, all of them, as the units
/// probe checks each against `FGJSBBase`.
pub mod units {
    pub use crate::units::*;
}

/// XML documents.
pub mod xml {
    pub use crate::xml::{DocumentLoader, Element, IncludeKind, XmlError, parse_document};
}
