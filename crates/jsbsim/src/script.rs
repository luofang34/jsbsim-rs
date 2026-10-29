// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/input_output/FGScript.{h,cpp}.
//
//! Scripts: a run time span and events that set properties when their
//! condition becomes true.

use crate::functions::{Condition, FnId, Parameter};
use crate::properties::{NodeId, NodeMap};

/// `FG_VALUE`, `FG_DELTA`, `FG_BOOL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetType {
    /// Set to the value.
    Value,
    /// Add the value to the current one.
    Delta,
    /// Set to the value (a boolean).
    Bool,
}

/// `FG_STEP`, `FG_RAMP`, `FG_EXP`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetAction {
    /// Immediately.
    Step,
    /// Linearly over `tc` seconds.
    Ramp,
    /// Exponentially with time constant `tc`.
    Exp,
}

/// One `<set>` of an event.
#[derive(Debug, Clone)]
pub struct Set {
    /// Property path.
    pub name: String,
    /// The node, once it exists.
    pub node: Option<NodeId>,
    /// Value function, instead of a constant.
    pub function: Option<FnId>,
    /// The constant, or the last function value.
    pub value: f64,
    /// Value before the event fired.
    pub original: f64,
    /// Target value.
    pub new_value: f64,
    /// `new_value - original`.
    pub span: f64,
    /// Type.
    pub kind: SetType,
    /// Action.
    pub action: SetAction,
    /// Time constant.
    pub tc: f64,
    /// Still moving towards the target.
    pub transiting: bool,
}

/// An `<event>`.
#[derive(Debug, Clone)]
pub struct Event {
    /// Name.
    pub name: String,
    /// Re-arms when the condition turns false.
    pub persistent: bool,
    /// Re-arms when the condition turns false, and steps keep following
    /// their function.
    pub continuous: bool,
    /// The condition.
    pub condition: Condition,
    /// Seconds between firing and acting.
    pub delay: f64,
    /// Whether to log when the event acts.
    pub notify: bool,
    /// Notification text.
    pub description: String,
    /// Properties logged with the notification, and their captions.
    pub notify_properties: Vec<(Parameter, String)>,
    /// The sets.
    pub sets: Vec<Set>,
    /// Fired and not re-armed.
    pub triggered: bool,
    /// Notified since firing.
    pub notified: bool,
    /// Time the sets start acting.
    pub start_time: f64,
}

impl Event {
    /// `event::reset`; sets keep their transiting flag, as in JSBSim.
    pub fn reset(&mut self) {
        self.triggered = false;
        self.notified = false;
        self.start_time = 0.0;
    }
}

/// `FGScript`.
#[derive(Debug, Clone, Default)]
pub struct Script {
    /// `runscript` name.
    pub name: String,
    /// Start time, s.
    pub start_time: f64,
    /// End time plus 0.99 frames, s.
    pub end_time: f64,
    /// Events in file order.
    pub events: Vec<Event>,
    /// `<property>` declarations of `<run>` and their initial values.
    pub local_properties: NodeMap<f64>,
    /// Set when `ResetEvents` runs while the events are out running, so
    /// the run resets them on its return.
    pub events_reset: bool,
}
