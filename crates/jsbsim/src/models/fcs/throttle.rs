// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGFCS.{h,cpp} (AddThrottle, bindThrottle
// and the engine command accessors).
//
//! The per-engine commands and positions FGFCS keeps: throttle, mixture,
//! propeller advance and feather, one of each per engine.

use crate::Error;
use crate::fdm::{Binder, Kind, Models};

/// The engine commands and positions, indexed by engine.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Throttles {
    /// `ThrottleCmd`.
    pub throttle_cmd: Vec<f64>,
    /// `ThrottlePos`.
    pub throttle_pos: Vec<f64>,
    /// `MixtureCmd`.
    pub mixture_cmd: Vec<f64>,
    /// `MixturePos`.
    pub mixture_pos: Vec<f64>,
    /// `PropAdvanceCmd`.
    pub prop_advance_cmd: Vec<f64>,
    /// `PropAdvance`.
    pub prop_advance: Vec<f64>,
    /// `PropFeatherCmd`.
    pub prop_feather_cmd: Vec<bool>,
    /// `PropFeather`.
    pub prop_feather: Vec<bool>,
}

/// `SetThrottleCmd` and the other setters: engine `n`, or every engine
/// for a negative `n`.
fn set_all<T: Copy>(v: &mut [T], n: i32, setting: T) {
    match usize::try_from(n) {
        Ok(i) => {
            if let Some(x) = v.get_mut(i) {
                *x = setting;
            }
        }
        Err(_) => v.iter_mut().for_each(|x| *x = setting),
    }
}

/// `SetMixturePos` and the other position setters: engine `n`, or, for a
/// negative `n`, every position copied from its command.
fn set_pos<T: Copy>(pos: &mut [T], cmd: &[T], n: i32, setting: T) {
    match usize::try_from(n) {
        Ok(i) => {
            if let Some(x) = pos.get_mut(i) {
                *x = setting;
            }
        }
        Err(_) => pos.iter_mut().zip(cmd).for_each(|(p, c)| *p = *c),
    }
}

impl Throttles {
    /// `AddThrottle` but for its ties: one more engine, at zero.
    pub fn add(&mut self) -> usize {
        self.throttle_cmd.push(0.0);
        self.throttle_pos.push(0.0);
        self.mixture_cmd.push(0.0);
        self.mixture_pos.push(0.0);
        self.prop_advance_cmd.push(0.0);
        self.prop_advance.push(0.0);
        self.prop_feather_cmd.push(false);
        self.prop_feather.push(false);
        self.throttle_cmd.len() - 1
    }

    /// `InitModel`'s part: positions and the throttle and mixture commands
    /// to zero; the advance and feather commands keep their values.
    pub fn init(&mut self) {
        for v in [
            &mut self.throttle_pos,
            &mut self.mixture_pos,
            &mut self.throttle_cmd,
            &mut self.mixture_cmd,
            &mut self.prop_advance,
        ] {
            v.fill(0.0);
        }
        self.prop_feather.fill(false);
    }

    /// `Run`'s default control law: each position follows its command.
    pub fn follow_commands(&mut self) {
        follow(&mut self.throttle_pos, &self.throttle_cmd);
        follow(&mut self.mixture_pos, &self.mixture_cmd);
        follow(&mut self.prop_advance, &self.prop_advance_cmd);
        follow(&mut self.prop_feather, &self.prop_feather_cmd);
    }

    /// `SetThrottleCmd`.
    pub fn set_throttle_cmd(&mut self, n: i32, setting: f64) {
        if self.exists(n, "throttle command") {
            set_all(&mut self.throttle_cmd, n, setting);
        }
    }

    /// `SetThrottlePos`.
    pub fn set_throttle_pos(&mut self, n: i32, setting: f64) {
        if self.exists(n, "throttle position setting") {
            set_all(&mut self.throttle_pos, n, setting);
        }
    }

    /// `SetMixtureCmd`.
    pub fn set_mixture_cmd(&mut self, n: i32, setting: f64) {
        set_all(&mut self.mixture_cmd, n, setting);
    }

    /// `SetMixturePos`.
    pub fn set_mixture_pos(&mut self, n: i32, setting: f64) {
        set_pos(&mut self.mixture_pos, &self.mixture_cmd, n, setting);
    }

    /// `SetPropAdvanceCmd`.
    pub fn set_prop_advance_cmd(&mut self, n: i32, setting: f64) {
        set_all(&mut self.prop_advance_cmd, n, setting);
    }

    /// `SetPropAdvance`.
    pub fn set_prop_advance(&mut self, n: i32, setting: f64) {
        set_pos(&mut self.prop_advance, &self.prop_advance_cmd, n, setting);
    }

    /// `SetFeatherCmd`.
    pub fn set_feather_cmd(&mut self, n: i32, setting: bool) {
        set_all(&mut self.prop_feather_cmd, n, setting);
    }

    /// `SetPropFeather`.
    pub fn set_prop_feather(&mut self, n: i32, setting: bool) {
        set_pos(&mut self.prop_feather, &self.prop_feather_cmd, n, setting);
    }

    /// Whether throttle `n` exists (all of them for a negative `n`), as the
    /// throttle setters check, logging what JSBSim logs when it does not.
    fn exists(&self, n: i32, what: &str) -> bool {
        let count = self.throttle_cmd.len();
        if usize::try_from(n).is_ok_and(|i| i >= count) {
            tracing::error!(
                "Throttle {n} does not exist! {count} engines exist, but attempted {what} is for engine {n}"
            );
            return false;
        }
        true
    }
}

/// Each position set to its command.
fn follow<T: Copy>(pos: &mut [T], cmd: &[T]) {
    pos.iter_mut().zip(cmd).for_each(|(p, c)| *p = *c);
}

type Get = fn(&Throttles) -> &[f64];
type Set = fn(&mut Throttles, i32, f64);

/// `bindThrottle(num)`: the engine's commands and positions, in JSBSim's
/// order.
pub fn bind_throttle(b: &mut Binder<'_>, num: usize) -> Result<(), Error> {
    let doubles: [(&str, Get, Set); 6] = [
        (
            "throttle-cmd-norm",
            |t| &t.throttle_cmd,
            Throttles::set_throttle_cmd,
        ),
        (
            "throttle-pos-norm",
            |t| &t.throttle_pos,
            Throttles::set_throttle_pos,
        ),
        (
            "mixture-cmd-norm",
            |t| &t.mixture_cmd,
            Throttles::set_mixture_cmd,
        ),
        (
            "mixture-pos-norm",
            |t| &t.mixture_pos,
            Throttles::set_mixture_pos,
        ),
        (
            "advance-cmd-norm",
            |t| &t.prop_advance_cmd,
            Throttles::set_prop_advance_cmd,
        ),
        (
            "advance-pos-norm",
            |t| &t.prop_advance,
            Throttles::set_prop_advance,
        ),
    ];
    let n = i32::try_from(num).unwrap_or(i32::MAX);
    for (name, get, set) in doubles {
        b.property(
            &format!("fcs/{name}[{num}]"),
            Kind::Double,
            move |m: &Models| get(&m.fcs.throttles).get(num).copied().unwrap_or(0.0),
            move |m, v| set(&mut m.fcs.throttles, n, v),
        )?;
    }
    let flag = move |b: &[bool]| b.get(num).map_or(0.0, |x| f64::from(u8::from(*x)));
    b.property(
        &format!("fcs/feather-cmd-norm[{num}]"),
        Kind::Bool,
        move |m| flag(&m.fcs.throttles.prop_feather_cmd),
        move |m, v| m.fcs.throttles.set_feather_cmd(n, v != 0.0),
    )?;
    b.property(
        &format!("fcs/feather-pos-norm[{num}]"),
        Kind::Bool,
        move |m| flag(&m.fcs.throttles.prop_feather),
        move |m, v| m.fcs.throttles.set_prop_feather(n, v != 0.0),
    )?;
    Ok(())
}

crate::snapshot::visit_fields! {
    Throttles {
        throttle_cmd, throttle_pos, mixture_cmd, mixture_pos, prop_advance_cmd, prop_advance,
        prop_feather_cmd, prop_feather,
    }
}
