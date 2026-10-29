// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGPropulsion.cpp (ConsumeFuel and the
// engine-wide commands).
//
//! Feeding the engines from their tanks by priority, and the commands
//! that reach every engine or the active one.

use super::engine::EngineType;
use super::{Propulsion, TankType};
use crate::Error;

/// The tanks one engine draws from this step.
#[derive(Debug, Clone, Default)]
pub(super) struct FeedLists {
    fuel: Vec<usize>,
    oxidizer: Vec<usize>,
}

impl FeedLists {
    /// Room for `n` feeds of each kind, so feeding never allocates.
    pub(super) fn reserve(&mut self, n: usize) {
        self.fuel.reserve(n);
        self.oxidizer.reserve(n);
    }
}

impl Propulsion {
    /// The tanks of `engine` with usable contents at the highest priority
    /// that has any, of `kind`, into `list`; whether there are any oxidizer
    /// tanks among the sources.
    fn feeds(&mut self, engine: usize, kind: TankType, oxidizer: bool) -> Result<bool, Error> {
        let Some(e) = self.engines.get(engine) else {
            return Ok(false);
        };
        let list = if oxidizer {
            &mut self.feed.oxidizer
        } else {
            &mut self.feed.fuel
        };
        list.clear();
        let mut has_tanks = false;
        let mut priority: u32 = 1;
        let count = u32::try_from(self.tanks.len()).unwrap_or(u32::MAX);
        while list.is_empty() && priority <= count {
            for &id in &e.base.source_tanks {
                let tank = usize::try_from(id)
                    .ok()
                    .and_then(|i| self.tanks.get(i).map(|t| (i, t)));
                let Some((i, t)) = tank else {
                    return Err(Error::Model {
                        message: format!(
                            "engine {engine} feeds from tank {id}, which does not exist"
                        ),
                    });
                };
                // JSBSim compares the priority as unsigned.
                let tank_priority = t.priority as u32;
                if tank_priority == 0 || t.tank_type != kind {
                    continue;
                }
                has_tanks = true;
                if t.contents > t.unusable() && t.selected && tank_priority == priority {
                    list.push(i);
                }
            }
            if list.is_empty() {
                priority = priority.wrapping_add(1);
            }
        }
        Ok(has_tanks)
    }

    /// `ConsumeFuel(engine)`: finds the tanks feeding it, sets whether it is
    /// starved, and drains what it needs from them equally.
    pub fn consume_fuel(&mut self, engine: usize, trim_status: bool) -> Result<(), Error> {
        if self.fuel_freeze || trim_status {
            return Ok(());
        }
        self.feeds(engine, TankType::Fuel, false)?;
        let fuel_starved = self.feed.fuel.is_empty();
        let rocket = self
            .engines
            .get(engine)
            .is_some_and(|e| e.kind.engine_type() == EngineType::Rocket);
        let (mut has_oxidizer_tanks, mut oxidizer_starved) = (false, true);
        if rocket {
            has_oxidizer_tanks = self.feeds(engine, TankType::Oxidizer, true)?;
            oxidizer_starved = self.feed.oxidizer.is_empty() || fuel_starved;
        } else {
            self.feed.oxidizer.clear();
        }
        let starved = fuel_starved || (has_oxidizer_tanks && oxidizer_starved);
        let Some(e) = self.engines.get_mut(engine) else {
            return Ok(());
        };
        e.base.starved = starved;
        if starved {
            return Ok(());
        }
        let fuel = e.kind.calc_fuel_need(&mut e.base, &self.input);
        let per_tank = fuel / self.feed.fuel.len() as f64;
        for &i in &self.feed.fuel {
            if let Some(t) = self.tanks.get_mut(i) {
                t.drain(per_tank);
            }
        }
        if rocket {
            let oxidizer = e.kind.calc_oxidizer_need(&mut e.base, &self.input);
            let n = self.feed.oxidizer.len();
            let per_tank = if n > 0 { oxidizer / n as f64 } else { 0.0 };
            for &i in &self.feed.oxidizer {
                if let Some(t) = self.tanks.get_mut(i) {
                    t.drain(per_tank);
                }
            }
        }
        Ok(())
    }

    /// `SetFuelFreeze`.
    pub fn set_fuel_freeze(&mut self, freeze: bool) {
        self.fuel_freeze = freeze;
        for engine in &mut self.engines {
            engine.base.fuel_freeze = freeze;
        }
    }

    /// The engines a command reaches: every one, or the active one.
    fn commanded(&mut self) -> &mut [super::Engine] {
        match usize::try_from(self.active_engine) {
            Ok(n) => self.engines.get_mut(n..=n).unwrap_or_default(),
            Err(_) => &mut self.engines,
        }
    }

    /// `SetMagnetos`: piston engines only.
    pub fn set_magnetos(&mut self, setting: i32) {
        for engine in self.commanded() {
            if engine.kind.engine_type() == EngineType::Piston {
                engine.kind.set_magnetos(setting);
            }
        }
    }

    /// `SetStarter`.
    pub fn set_starter(&mut self, setting: i32) {
        for engine in self.commanded() {
            engine.base.starter = setting != 0;
        }
    }

    /// `GetStarter`: 1 when every commanded engine's starter is on.
    #[must_use]
    pub fn starter(&self) -> i32 {
        let on = match usize::try_from(self.active_engine) {
            Ok(n) => self.engines.get(n).is_some_and(|e| e.base.starter),
            Err(_) => self.engines.iter().all(|e| e.base.starter),
        };
        i32::from(on)
    }

    /// `SetCutoff`: turbine and turboprop engines only.
    pub fn set_cutoff(&mut self, setting: i32) {
        for engine in self.commanded() {
            if matches!(
                engine.kind.engine_type(),
                EngineType::Turbine | EngineType::Turboprop
            ) {
                engine.kind.set_cutoff(setting != 0);
            }
        }
    }

    /// `GetCutoff`: 1 when every commanded engine is cut off, -1 when one
    /// has no cutoff.
    #[must_use]
    pub fn cutoff(&self) -> i32 {
        let cutoff = |e: &super::Engine| match e.kind.engine_type() {
            EngineType::Turbine | EngineType::Turboprop => e.kind.cutoff(),
            _ => None,
        };
        match usize::try_from(self.active_engine) {
            Ok(n) => match self.engines.get(n).and_then(cutoff) {
                Some(c) => i32::from(c),
                None => -1,
            },
            Err(_) => {
                let mut all = true;
                for e in &self.engines {
                    match cutoff(e) {
                        Some(c) => all &= c,
                        None => return -1,
                    }
                }
                i32::from(all)
            }
        }
    }

    /// `SetActiveEngine`: -1 for all engines when `engine` is not one.
    pub fn set_active_engine(&mut self, engine: i32) {
        let n = i32::try_from(self.engines.len()).unwrap_or(i32::MAX);
        self.active_engine = if engine >= n || engine < 0 {
            -1
        } else {
            engine
        };
    }
}
