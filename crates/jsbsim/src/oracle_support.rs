// SPDX-License-Identifier: LGPL-2.1-or-later
//! Internals that `tools/oracle` reaches, compiled only with the
//! `__oracle` feature. **Not part of the supported API:** anything here may
//! change or vanish in any release.
//!
//! The oracle's probes mirror JSBSim's C++ classes one for one and compare
//! each against its C++ twin, so they need parts of the port that a flight
//! simulation front end never does. Each submodule re-exports, under the
//! crate-internal module's name, exactly the items the oracle uses;
//! [`FdmInternals`] adds the executive's internal entry points, and the
//! feature also makes the `Fdm::models` field public.
//!
//! What the oracle reaches this way has to be `pub`, so a build without the
//! feature finds it unreachable, and perhaps unused. Each such item, or the
//! `impl` that holds it, allows those lints for that build alone with
//! `cfg_attr(not(feature = "__oracle"), allow(..))`; everything else is
//! `pub(crate)` or narrower.

use crate::{Error, Fdm, Resolver};

/// The executive's internal entry points the probes drive.
pub trait FdmInternals {
    /// The property tree.
    fn tree(&self) -> &properties::PropertyTree;
    /// `FGMassBalance::Load(<mass_balance>)`, tying the point mass
    /// properties as it adds them.
    fn load_mass_balance(&mut self, el: &xml::Element) -> Result<(), Error>;
    /// `LoadPlanet(PlanetPath, useAircraftPath)`.
    fn load_planet_file(
        &mut self,
        resolver: &dyn Resolver,
        path: &str,
        use_aircraft_path: bool,
    ) -> Result<(), Error>;
    /// `Atmosphere->InitModel()`.
    fn init_atmosphere(&mut self) -> Result<(), Error>;
    /// `Atmosphere->Run(holding)` on the inputs already loaded.
    fn run_atmosphere(&mut self, holding: bool) -> Result<(), Error>;
    /// `FGPropulsion::GetSteadyState`.
    fn steady_state(&mut self) -> Result<bool, Error>;
    /// `FGLinearization(fdm)` about the current state, which leaves the time
    /// step at zero as JSBSim does.
    fn linearize(&mut self) -> Result<crate::linearization::Linearization, Error>;
}

impl FdmInternals for Fdm {
    fn tree(&self) -> &properties::PropertyTree {
        Self::tree(self)
    }
    fn load_mass_balance(&mut self, el: &xml::Element) -> Result<(), Error> {
        Self::load_mass_balance(self, el)
    }
    fn load_planet_file(
        &mut self,
        resolver: &dyn Resolver,
        path: &str,
        use_aircraft_path: bool,
    ) -> Result<(), Error> {
        Self::load_planet_file(self, resolver, path, use_aircraft_path)
    }
    fn init_atmosphere(&mut self) -> Result<(), Error> {
        Self::init_atmosphere(self)
    }
    fn run_atmosphere(&mut self, holding: bool) -> Result<(), Error> {
        Self::run_atmosphere(self, holding)
    }
    fn steady_state(&mut self) -> Result<bool, Error> {
        Self::steady_state(self)
    }
    fn linearize(&mut self) -> Result<crate::linearization::Linearization, Error> {
        Self::linearize(self)
    }
}

/// Tracing target of the event a `Location` or `Quaternion` emits when a
/// derived value is read without its owner having called `cache_derived`.
pub const UNCACHED_DERIVED_TARGET: &str = crate::math::UNCACHED_DERIVED_TARGET;

/// `FGTrim`.
pub mod trim {
    pub use crate::trim::trimmer::Trim;
}

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

/// The unit conversion constants and functions the units probe checks
/// against `FGJSBBase`.
pub mod units {
    pub use crate::units::*;
}

/// XML documents.
pub mod xml {
    pub use crate::xml::{DocumentLoader, Element, IncludeKind, XmlError, parse_document};
}
