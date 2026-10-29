// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/propulsion/FGEngine.cpp (LoadThruster)
// and src/models/propulsion/FGThruster.h.
//
//! The thruster types and their dispatch; see [`super`] for how a type
//! is registered.

use crate::Error;
use crate::fdm::ModelEvaluator;
use crate::fdm::{LoadContext, Models};
use crate::models::propulsion::engine::Engine;
use crate::models::propulsion::thruster::{Thruster, ThrusterBase};
use crate::xml::Element;

/// What a thruster type's constructor reads from the executive besides
/// its elements.
pub struct ThrusterSite<'a> {
    /// The executive's step when the thruster loads (`GetDeltaT()`), s.
    pub dt: f64,
    /// The engines loaded before this thruster's.
    pub engines: &'a [Engine],
}

/// One thruster type (`FGThruster` or a subclass).
pub trait ThrusterFamily: Sized {
    /// What one thrust calculation reads from the property tree.
    type Sample;

    /// The type's constructor on its element `el` (the child of
    /// `thruster_el`, the `<thruster>`), once `FGThruster`'s constructor
    /// has filled `base`.
    fn load(
        el: &Element,
        thruster_el: &Element,
        base: &mut ThrusterBase,
        ctx: &mut LoadContext<'_>,
        site: &ThrusterSite<'_>,
    ) -> Result<Self, Error>;

    /// The reads of `Calculate`, from the property tree through `ev` and
    /// from the other models.
    fn sample(
        &self,
        base: &ThrusterBase,
        models: &Models,
        ev: &mut ModelEvaluator<'_>,
    ) -> Result<Self::Sample, Error>;

    /// `Calculate(power)`: the thrust, lbs.
    fn calculate(&mut self, base: &mut ThrusterBase, power: f64, sample: &Self::Sample) -> f64;

    /// `ResetToIC`.
    fn reset_to_ic(&mut self, base: &mut ThrusterBase) {
        base.reset_to_ic();
    }

    /// `GetRPM`: the thruster's own speed, rpm.
    fn rpm(&self) -> f64 {
        0.0
    }

    /// `GetEngineRPM`: the speed it turns the engine at, rpm.
    fn engine_rpm(&self, _base: &ThrusterBase) -> f64 {
        0.0
    }

    /// `SetRPM`.
    fn set_rpm(&mut self, _rpm: f64) {}

    /// `SetEngineRPM`.
    fn set_engine_rpm(&mut self, _base: &ThrusterBase, _rpm: f64) {}

    /// `FGPropeller::SetAdvance`, which engines call on a propeller only.
    fn set_advance(&mut self, _advance: f64) {}

    /// `FGPropeller::SetFeather`.
    fn set_feather(&mut self, _feather: bool) {}

    /// Whether it is a propeller (`GetType() == ttPropeller`), which
    /// engines set the propeller's controls on.
    fn is_propeller(&self) -> bool {
        false
    }

    /// `FGPropeller::SetReverse`.
    fn set_reverse(&mut self, _reversed: bool) {}

    /// `FGPropeller::SetReverseCoef`.
    fn set_reverse_coef(&mut self, _coef: f64) {}

    /// `GetTorque` of a propeller: the engine torque on the shaft, lbs·ft.
    fn torque(&self) -> f64 {
        0.0
    }
}

macro_rules! thrusters {
    ($($(#[$cfg:meta])* $variant:ident($ty:ty) = $element:literal;)*) => {
        /// A thruster's type, with the type's own state.
        // Each engine holds one in place, read every frame; boxing the large
        // variants would only add an indirection.
        #[allow(clippy::large_enum_variant)]
        #[derive(Debug, Clone)]
        pub enum ThrusterKind {
            $($(#[$cfg])* #[doc = concat!("`<", $element, ">`.")] $variant($ty),)*
        }

        /// What a thruster's [`ThrusterFamily::sample`] read.
        #[derive(Debug)]
        pub enum ThrusterSample {
            $($(#[$cfg])* #[doc = concat!("`<", $element, ">`.")]
            $variant(<$ty as ThrusterFamily>::Sample),)*
        }

        /// The elements thruster types register, in registration order.
        pub const REGISTERED_THRUSTERS: &[&str] = &[$($(#[$cfg])* $element,)*];

        /// Whether a thruster type registers element `name`.
        #[must_use]
        pub fn thruster_registered(name: &str) -> bool {
            match name {
                $($(#[$cfg])* $element => true,)*
                _ => false,
            }
        }

        /// The thruster type registered for element `name` (the child `el`
        /// of `<thruster>`), loaded onto `base`; `None` when no type
        /// registers `name`.
        #[allow(unused_variables, unused_mut)]
        pub(crate) fn load_thruster(
            name: &str,
            el: &Element,
            thruster_el: &Element,
            mut base: ThrusterBase,
            ctx: &mut LoadContext<'_>,
            site: &ThrusterSite<'_>,
        ) -> Result<Option<Thruster>, Error> {
            match name {
                $($(#[$cfg])* $element => {
                    let family = <$ty as ThrusterFamily>::load(el, thruster_el, &mut base, ctx, site)?;
                    Ok(Some(Thruster { base, kind: ThrusterKind::$variant(family) }))
                })*
                _ => Ok(None),
            }
        }

        impl crate::snapshot::Visit for ThrusterKind {
            fn visit<V: crate::snapshot::Visitor>(
                &mut self,
                v: &mut V,
            ) -> Result<(), crate::snapshot::SnapshotError> {
                match self {
                    $($(#[$cfg])* Self::$variant(f) => {
                        v.kind($element);
                        crate::snapshot::Visit::visit(f, v)
                    })*
                }
            }
        }

        // With no type registered the arguments of these dispatches go
        // unused.
        #[allow(unused_variables)]
        impl Thruster {
            /// The reads of the thruster's `Calculate`.
            pub fn sample(
                &self,
                models: &Models,
                ev: &mut ModelEvaluator<'_>,
            ) -> Result<ThrusterSample, Error> {
                match self.kind {
                    $($(#[$cfg])* ThrusterKind::$variant(ref f) => {
                        f.sample(&self.base, models, ev).map(ThrusterSample::$variant)
                    })*
                }
            }

            /// `Calculate(power)` on what [`Thruster::sample`] read: the
            /// thrust, lbs.
            pub fn calculate(&mut self, power: f64, sample: &ThrusterSample) -> f64 {
                match (&mut self.kind, sample) {
                    $($(#[$cfg])* (ThrusterKind::$variant(f), ThrusterSample::$variant(s)) => {
                        f.calculate(&mut self.base, power, s)
                    })*
                    #[allow(unreachable_patterns)]
                    _ => self.base.thrust,
                }
            }

            /// `ResetToIC`.
            pub fn reset_to_ic(&mut self) {
                match self.kind {
                    $($(#[$cfg])* ThrusterKind::$variant(ref mut f) => f.reset_to_ic(&mut self.base),)*
                }
            }

            /// The element its type registers.
            #[must_use]
            pub fn element(&self) -> &'static str {
                match self.kind {
                    $($(#[$cfg])* ThrusterKind::$variant(_) => $element,)*
                }
            }

            /// `GetRPM`.
            #[must_use]
            pub fn rpm(&self) -> f64 {
                match self.kind {
                    $($(#[$cfg])* ThrusterKind::$variant(ref f) => f.rpm(),)*
                }
            }

            /// `GetEngineRPM`.
            #[must_use]
            pub fn engine_rpm(&self) -> f64 {
                match self.kind {
                    $($(#[$cfg])* ThrusterKind::$variant(ref f) => f.engine_rpm(&self.base),)*
                }
            }

            /// `SetRPM`.
            pub fn set_rpm(&mut self, rpm: f64) {
                match self.kind {
                    $($(#[$cfg])* ThrusterKind::$variant(ref mut f) => f.set_rpm(rpm),)*
                }
            }

            /// `SetEngineRPM`.
            pub fn set_engine_rpm(&mut self, rpm: f64) {
                match self.kind {
                    $($(#[$cfg])* ThrusterKind::$variant(ref mut f) => f.set_engine_rpm(&self.base, rpm),)*
                }
            }

            /// `SetAdvance` of a propeller.
            pub fn set_advance(&mut self, advance: f64) {
                match self.kind {
                    $($(#[$cfg])* ThrusterKind::$variant(ref mut f) => f.set_advance(advance),)*
                }
            }

            /// `SetFeather` of a propeller.
            pub fn set_feather(&mut self, feather: bool) {
                match self.kind {
                    $($(#[$cfg])* ThrusterKind::$variant(ref mut f) => f.set_feather(feather),)*
                }
            }

            /// Whether it is a propeller.
            #[must_use]
            pub fn is_propeller(&self) -> bool {
                match self.kind {
                    $($(#[$cfg])* ThrusterKind::$variant(ref f) => f.is_propeller(),)*
                }
            }

            /// `SetReverse` of a propeller.
            pub fn set_reverse(&mut self, reversed: bool) {
                match self.kind {
                    $($(#[$cfg])* ThrusterKind::$variant(ref mut f) => f.set_reverse(reversed),)*
                }
            }

            /// `SetReverseCoef` of a propeller.
            pub fn set_reverse_coef(&mut self, coef: f64) {
                match self.kind {
                    $($(#[$cfg])* ThrusterKind::$variant(ref mut f) => f.set_reverse_coef(coef),)*
                }
            }

            /// `GetTorque` of a propeller.
            #[must_use]
            pub fn torque(&self) -> f64 {
                match self.kind {
                    $($(#[$cfg])* ThrusterKind::$variant(ref f) => f.torque(),)*
                }
            }
        }
    };
}

thrusters! {
    Propeller(crate::models::propulsion::propeller::Propeller) = "propeller";
    Nozzle(crate::models::propulsion::nozzle::Nozzle) = "nozzle";
    Rotor(crate::models::propulsion::rotor::Rotor) = "rotor";
    Direct(crate::models::propulsion::thruster::Direct) = "direct";
    #[cfg(test)]
    Test(crate::models::propulsion::test_engine::TestThruster) = "test_thruster";
}

/// Thruster elements the tests look for after JSBSim's own: one registered
/// only by them, and one no type registers.
#[cfg(test)]
pub(crate) const TEST_THRUSTER_ELEMENTS: [&str; 2] = ["test_thruster", "unregistered_thruster"];
