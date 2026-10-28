// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2000-2005 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/FGFCS.cpp (Load's component dispatch)
// and src/models/FGFCSChannel.h (Execute).
//
//! The component families and their dispatch.
//!
//! A family is a module beside this one (declared in `fcs.rs`) with a type
//! implementing [`Family`], registered by one line in the `families!`
//! invocation at the end of this file:
//!
//! ```text
//! /// `<kinematic>` (`FGKinemat`).
//! Kinematic(super::kinematic::Kinemat) = "kinematic";
//! ```
//!
//! naming the enum variant, the type and the element names it loads. The
//! macro adds the variant to [`Kind`], the element names to the loader's
//! dispatch and the family to the per-frame dispatch; nothing else needs
//! editing. A registered element name takes precedence over the list of
//! still-unsupported names in `load.rs`.
//!
//! A run has two phases, so a family never holds its own state while the
//! property tree (and through it every model's getters) is read:
//! [`Family::sample`] reads everything the C++ `Run` reads, through an
//! [`Evaluator`] over `&Models`, into an owned `Sample`; [`Family::step`]
//! then updates the state from it without reading anything. Both run
//! without allocating.

use super::Fcs;
use super::component::{Base, Tick};
use crate::Error;
use crate::fdm::{Env, LoadContext, Models};
use crate::functions::Evaluator;
use crate::xml::Element;

/// Where a component sits: channel index and index in the channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentAt {
    /// Channel, in load order across every FCS document.
    pub channel: usize,
    /// Component, in channel order.
    pub index: usize,
}

/// One component family (a subclass of `FGFCSComponent`).
pub trait Family: Sized {
    /// What one run reads from the property tree.
    type Sample;

    /// The family's constructor, after [`Base::load`]; it calls
    /// [`Base::bind`] where the C++ constructor calls `bind`. `at` is where
    /// the component will sit, for ties that reach its state.
    fn load(
        el: &Element,
        base: &mut Base,
        ctx: &mut LoadContext<'_>,
        at: ComponentAt,
    ) -> Result<Self, Error>;

    /// The reads of `Run`, in its order.
    fn sample(&self, base: &Base, ev: &mut Evaluator<'_>) -> Result<Self::Sample, Error>;

    /// [`Family::sample`] for a family that also reads the models
    /// directly, as the inertial sensors read accelerations and rates.
    fn sample_models(
        &self,
        base: &Base,
        _models: &Models,
        ev: &mut Evaluator<'_>,
    ) -> Result<Self::Sample, Error> {
        self.sample(base, ev)
    }

    /// The rest of `Run` up to `SetOutput`: `base.output` is what the
    /// outputs receive.
    fn step(&mut self, base: &mut Base, sample: Self::Sample, tick: &Tick);

    /// `ResetPastStates`.
    fn reset(&mut self, base: &mut Base) {
        base.reset_past_states();
    }
}

/// A loaded component.
#[derive(Debug, Clone)]
pub struct Component {
    /// What every component has.
    pub base: Base,
    /// The family and its state.
    pub kind: Kind,
}

impl Component {
    /// `ResetPastStates`.
    pub fn reset(&mut self) {
        self.kind.reset(&mut self.base);
    }
}

fn component(fcs: &Fcs, at: ComponentAt) -> Option<&Component> {
    fcs.channels.get(at.channel)?.components.get(at.index)
}

/// The component at `at`, for a tied setter that reaches its state.
pub fn component_mut(fcs: &mut Fcs, at: ComponentAt) -> Option<&mut Component> {
    fcs.channels
        .get_mut(at.channel)?
        .components
        .get_mut(at.index)
}

macro_rules! families {
    ($($(#[$doc:meta])* $variant:ident($ty:ty) = $($element:literal)|+;)+) => {
        /// A component's family, with the family's own state.
        #[derive(Debug, Clone)]
        pub enum Kind {
            $($(#[$doc])* $variant($ty),)+
        }

        impl Kind {
            fn reset(&mut self, base: &mut Base) {
                match self {
                    $(Self::$variant(family) => family.reset(base),)+
                }
            }
        }

        /// Whether a family registers element `name`.
        #[must_use]
        pub fn registers(name: &str) -> bool {
            matches!(name, $($($element)|+)|+)
        }

        /// The family registered for `el`'s name, loaded.
        pub fn load(
            el: &Element,
            base: &mut Base,
            ctx: &mut LoadContext<'_>,
            at: ComponentAt,
        ) -> Result<Kind, Error> {
            match el.name() {
                $($($element)|+ => <$ty as Family>::load(el, base, ctx, at).map(Kind::$variant),)+
                name => Err(Error::Unsupported {
                    construct: format!("<{name}> FCS component"),
                    path: el.location().to_string(),
                }),
            }
        }

        /// Runs the component at `at` up to `SetOutput`; false when there
        /// is none.
        pub(crate) fn run(env: &mut Env, models: &mut Models, at: ComponentAt) -> Result<bool, Error> {
            let trim_status = models.exec.trim_status;
            let Some(c) = component(&models.fcs, at) else {
                return Ok(false);
            };
            match &c.kind {
                $(Kind::$variant(family) => {
                    let base = &c.base;
                    let (sample, clip) = env.eval(models, |ev| {
                        let sample = family.sample_models(base, models, ev)?;
                        Ok((sample, base.clip_limits(ev)?))
                    })?;
                    let tick = Tick { clip, trim_status };
                    if let Some(Component { base, kind: Kind::$variant(family) }) =
                        component_mut(&mut models.fcs, at)
                    {
                        family.step(base, sample, &tick);
                    }
                })+
            }
            Ok(true)
        }
    };
}

families! {
    /// `<fcs_function>` (`FGFCSFunction`).
    Function(super::function::FcsFunction) = "fcs_function";
    /// `<pure_gain>`, `<scheduled_gain>` and `<aerosurface_scale>` (`FGGain`).
    Gain(super::gain::Gain) = "pure_gain" | "scheduled_gain" | "aerosurface_scale";
    /// `<summer>` (`FGSummer`).
    Summer(super::summer::Summer) = "summer";
    /// `<deadband>` (`FGDeadBand`).
    DeadBand(super::deadband::DeadBand) = "deadband";
    /// `<switch>` (`FGSwitch`).
    Switch(super::switch::Switch) = "switch";
    /// `<lag_filter>`, `<lead_lag_filter>`, `<second_order_filter>` and
    /// `<washout_filter>` (`FGFilter`).
    Filter(super::filter::Filter) =
        "lag_filter" | "lead_lag_filter" | "second_order_filter" | "washout_filter";
    /// `<pid>` and `<integrator>` (`FGPID`).
    Pid(super::pid::Pid) = "pid" | "integrator";
    /// `<sensor>` (`FGSensor`).
    Sensor(super::sensor::Sensor) = "sensor";
    /// `<accelerometer>` (`FGAccelerometer`).
    Accelerometer(super::accelerometer::Accelerometer) = "accelerometer";
    /// `<gyro>` (`FGGyro`).
    Gyro(super::gyro::Gyro) = "gyro";
    /// `<magnetometer>` (`FGMagnetometer`).
    Magnetometer(super::magnetometer::Magnetometer) = "magnetometer";
    /// `<waypoint_heading>` and `<waypoint_distance>` (`FGWaypoint`).
    Waypoint(super::waypoint::Waypoint) = "waypoint_heading" | "waypoint_distance";
}
