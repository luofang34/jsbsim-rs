// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGPropulsion.cpp (Load's engine dispatch)
// and src/models/propulsion/FGEngine.cpp (LoadThruster).
//
//! The engine and thruster types and their dispatch.
//!
//! An engine type is a module beside this one (declared in
//! `propulsion.rs`) with a type implementing [`EngineFamily`],
//! registered by one line in the `engines!` invocation at the end of this
//! file:
//!
//! ```text
//! Piston(super::piston::Piston) = "piston_engine";
//! ```
//!
//! naming the enum variant, the type and the element JSBSim finds in
//! `<engine>` (or in the engine file it includes). A thruster type is
//! registered the same way in `thrusters!`, implementing
//! [`ThrusterFamily`]:
//!
//! ```text
//! Propeller(super::propeller::Propeller) = "propeller";
//! ```
//!
//! The macros add the variant to [`EngineKind`] or [`ThrusterKind`], the
//! element to the loader's dispatch and the type to the per-frame
//! dispatch; nothing else needs editing. JSBSim looks for the element
//! names of [`ENGINE_ELEMENTS`] and [`THRUSTER_ELEMENTS`] in that order;
//! the first one present that is not registered is refused with
//! [`Error::Unsupported`] naming the element and where it is.
//!
//! An engine's constructor ([`EngineFamily::load`]) calls
//! [`EngineLoad::load_base`] where the C++ constructor calls
//! `FGEngine::Load`; that loads the engine's functions, its thruster
//! (through [`ThrusterFamily::load`]), its feed tanks and its
//! `propulsion/engine[n]/` properties.
//!
//! `Calculate` has two phases, as FCS components do (see
//! [`crate::models::fcs::families`]), so an engine never holds its own
//! state while the property tree, and through it every model's getters,
//! is read: [`EngineFamily::sample`] reads everything the C++ `Calculate`
//! reads, through a [`ModelEvaluator`] over `&Models`, into an owned
//! `Sample` (asking its thruster for the thruster's reads with
//! [`Thruster::sample`], which reads the other models too); [`EngineFamily::step`] then updates the engine
//! and calls [`Thruster::calculate`] without reading anything. The
//! engine's pre- and post-functions run around the two. Both phases run
//! without allocating.

mod thrusters;

#[cfg(test)]
pub(crate) use thrusters::TEST_THRUSTER_ELEMENTS;
pub(crate) use thrusters::load_thruster;
pub use thrusters::{ThrusterFamily, ThrusterKind, ThrusterSample, thruster_registered};

use super::Inputs;
use super::engine::{EngineBase, EngineType};
use super::thruster::Thruster;
use crate::Error;
use crate::fdm::ModelEvaluator;
use crate::fdm::{EngineLoad, Env, Models};
use crate::xml::Element;

/// The engine elements JSBSim's `FGPropulsion::Load` looks for, in order.
pub const ENGINE_ELEMENTS: [&str; 6] = [
    "piston_engine",
    "turbine_engine",
    "turboprop_engine",
    "rocket_engine",
    "electric_engine",
    "brushless_dc_motor",
];

/// The thruster elements JSBSim's `FGEngine::LoadThruster` looks for, in
/// order.
pub const THRUSTER_ELEMENTS: [&str; 4] = ["propeller", "nozzle", "rotor", "direct"];

/// What an engine type's `InitRunning` leaves to the executive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitRunning {
    /// Nothing: the engine is running.
    Done,
    /// A `Calculate` of the engine, then [`EngineFamily::init_running_done`]:
    /// with integration suspended (`FGTurbine`), or with the engine inputs'
    /// step set to 0 for it (`FGTurboProp`).
    Calculate {
        /// `SuspendIntegration` around it.
        suspend_integration: bool,
        /// `in.TotalDeltaT = 0` around it.
        zero_step: bool,
    },
}

/// One engine type (a subclass of `FGEngine`).
pub trait EngineFamily: Sized {
    /// Its `FGEngine::EngineType`.
    const TYPE: EngineType;
    /// What one `Calculate` reads from the property tree.
    type Sample;

    /// The type's constructor on its element `el`.
    fn load(el: &Element, cx: &mut EngineLoad<'_>) -> Result<Self, Error>;

    /// The reads of `Calculate`, in its order.
    fn sample(
        &self,
        base: &EngineBase,
        thruster: &Thruster,
        models: &Models,
        ev: &mut ModelEvaluator<'_>,
    ) -> Result<Self::Sample, Error>;

    /// The rest of `Calculate`.
    fn step(
        &mut self,
        base: &mut EngineBase,
        thruster: &mut Thruster,
        sample: Self::Sample,
        input: &Inputs,
    );

    /// `CalcFuelNeed`.
    fn calc_fuel_need(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
        base.calc_fuel_need(input)
    }

    /// `CalcOxidizerNeed`.
    fn calc_oxidizer_need(&mut self, _base: &mut EngineBase, _input: &Inputs) -> f64 {
        0.0
    }

    /// `InitRunning`, which may change the engine inputs, up to the
    /// `Calculate` it may ask for.
    fn init_running(
        &mut self,
        _base: &mut EngineBase,
        _thruster: &mut Thruster,
        _input: &mut Inputs,
    ) -> InitRunning {
        InitRunning::Done
    }

    /// The rest of `InitRunning` after the `Calculate` it asked for.
    fn init_running_done(&mut self) {}

    /// `ResetToIC`, which also resets the thruster.
    fn reset_to_ic(&mut self, base: &mut EngineBase, thruster: &mut Thruster, _input: &Inputs) {
        base.reset_to_ic();
        thruster.reset_to_ic();
    }

    /// `FGPiston::SetMagnetos`; other types have no magnetos.
    fn set_magnetos(&mut self, _setting: i32) {}

    /// The cutoff of a type that has one (`FGTurbine`, `FGTurboProp`).
    fn cutoff(&self) -> Option<bool> {
        None
    }

    /// `SetCutoff` of a type that has one.
    fn set_cutoff(&mut self, _cutoff: bool) {}
}

macro_rules! engines {
    ($($(#[$cfg:meta])* $variant:ident($ty:ty) = $element:literal;)*) => {
        /// An engine's type, with the type's own state.
        // Each engine holds one in place, read every frame; boxing the large
        // variants would only add an indirection.
        #[allow(clippy::large_enum_variant)]
        #[derive(Debug, Clone)]
        pub enum EngineKind {
            $($(#[$cfg])* #[doc = concat!("`<", $element, ">`.")] $variant($ty),)*
        }

        /// Whether an engine type registers element `name`.
        #[must_use]
        pub fn engine_registered(name: &str) -> bool {
            match name {
                $($(#[$cfg])* $element => true,)*
                _ => false,
            }
        }

        // With no type registered the arguments of these dispatches go
        // unused.
        #[allow(unused_variables)]
        impl EngineKind {
            /// Its `FGEngine::EngineType`.
            #[must_use]
            pub fn engine_type(&self) -> EngineType {
                match *self {
                    $($(#[$cfg])* Self::$variant(_) => <$ty as EngineFamily>::TYPE,)*
                }
            }

            /// `CalcFuelNeed`.
            pub fn calc_fuel_need(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
                match *self {
                    $($(#[$cfg])* Self::$variant(ref mut f) => f.calc_fuel_need(base, input),)*
                }
            }

            /// `CalcOxidizerNeed`.
            pub fn calc_oxidizer_need(&mut self, base: &mut EngineBase, input: &Inputs) -> f64 {
                match *self {
                    $($(#[$cfg])* Self::$variant(ref mut f) => f.calc_oxidizer_need(base, input),)*
                }
            }

            /// `InitRunning`.
            pub fn init_running(
                &mut self,
                base: &mut EngineBase,
                thruster: &mut Thruster,
                input: &mut Inputs,
            ) -> InitRunning {
                match *self {
                    $($(#[$cfg])* Self::$variant(ref mut f) => f.init_running(base, thruster, input),)*
                }
            }

            /// The rest of `InitRunning` after its `Calculate`.
            pub fn init_running_done(&mut self) {
                match *self {
                    $($(#[$cfg])* Self::$variant(ref mut f) => f.init_running_done(),)*
                }
            }

            /// `ResetToIC`.
            pub fn reset_to_ic(&mut self, base: &mut EngineBase, thruster: &mut Thruster, input: &Inputs) {
                match *self {
                    $($(#[$cfg])* Self::$variant(ref mut f) => f.reset_to_ic(base, thruster, input),)*
                }
            }

            /// `SetMagnetos`.
            pub fn set_magnetos(&mut self, setting: i32) {
                match *self {
                    $($(#[$cfg])* Self::$variant(ref mut f) => f.set_magnetos(setting),)*
                }
            }

            /// The cutoff, for a type that has one.
            #[must_use]
            pub fn cutoff(&self) -> Option<bool> {
                match *self {
                    $($(#[$cfg])* Self::$variant(ref f) => f.cutoff(),)*
                }
            }

            /// `SetCutoff`.
            pub fn set_cutoff(&mut self, cutoff: bool) {
                match *self {
                    $($(#[$cfg])* Self::$variant(ref mut f) => f.set_cutoff(cutoff),)*
                }
            }
        }

        /// Constructs the engine type registered for element `name` (the
        /// child `el` of `<engine>`) and appends it; false when no type
        /// registers `name`.
        #[allow(unused_variables)]
        pub(crate) fn load_engine(
            name: &str,
            el: &Element,
            cx: &mut EngineLoad<'_>,
        ) -> Result<bool, Error> {
            match name {
                $($(#[$cfg])* $element => {
                    let family = <$ty as EngineFamily>::load(el, cx)?;
                    cx.push(<$ty as EngineFamily>::TYPE, |base, thruster, engines| {
                        engines.push(super::engine::Engine {
                            base,
                            thruster,
                            kind: EngineKind::$variant(family),
                        });
                    })?;
                    Ok(true)
                })*
                _ => Ok(false),
            }
        }

        /// `Calculate` of engine `i` between its pre- and post-functions.
        #[allow(unused_variables)]
        pub(crate) fn calculate(env: &mut Env, models: &mut Models, i: usize) -> Result<(), Error> {
            let Some(engine) = models.propulsion.engines.get(i) else {
                return Ok(());
            };
            match engine.kind {
                $($(#[$cfg])* EngineKind::$variant(ref family) => {
                    let (base, thruster) = (&engine.base, &engine.thruster);
                    let sample = env.eval(models, |ev| family.sample(base, thruster, models, ev))?;
                    let p = &mut models.propulsion;
                    if let Some(super::engine::Engine {
                        base,
                        thruster,
                        kind: EngineKind::$variant(family),
                    }) = p.engines.get_mut(i)
                    {
                        family.step(base, thruster, sample, &p.input);
                    }
                    Ok(())
                })*
            }
        }
    };
}

engines! {
    Piston(super::piston::Piston) = "piston_engine";
    Turbine(super::turbine::Turbine) = "turbine_engine";
    TurboProp(super::turboprop::TurboProp) = "turboprop_engine";
    Electric(super::electric::Electric) = "electric_engine";
    #[cfg(test)]
    Test(super::test_engine::TestEngine) = "test_engine";
}

/// Engine elements registered only by the tests, after JSBSim's own.
#[cfg(test)]
pub(crate) const TEST_ENGINE_ELEMENTS: [&str; 1] = ["test_engine"];
