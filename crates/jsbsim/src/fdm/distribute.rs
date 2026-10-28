// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 2013 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/flight_control/FGDistributor.cpp (Run).
//
//! `FGDistributor::Run`, which writes properties between its reads (see
//! [`crate::models::fcs::distributor`]).

use super::Fdm;
use crate::Error;
use crate::models::fcs::{ComponentAt, distributor};

impl Fdm {
    /// `FGDistributor::Run` when the component at `at` is a distributor.
    /// Each value is written before the next test or value is read, and
    /// the distributor is looked up afresh after every write, since a
    /// setter's request may reset the executive.
    pub(super) fn run_distributor(&mut self, at: ComponentAt) -> Result<(), Error> {
        let Some(cases) = distributor::cases(&self.models.fcs, at) else {
            return Ok(());
        };
        let mut completed = false;
        for case in 0..cases {
            let m = &self.models;
            let (sets, completes) = self.env.eval(m, |ev| match distributor::at(&m.fcs, at) {
                Some(d) => d.fires(case, completed, ev),
                None => Ok((false, false)),
            })?;
            if !sets {
                continue;
            }
            for pair in 0..distributor::pairs(&self.models.fcs, at, case) {
                let m = &self.models;
                let target = self.env.eval(m, |ev| match distributor::at(&m.fcs, at) {
                    Some(d) => d.pair(case, pair, ev),
                    None => Ok(None),
                })?;
                if let Some((node, value)) = target {
                    self.write_node(node, value)?;
                }
            }
            completed |= completes;
        }
        Ok(())
    }
}
