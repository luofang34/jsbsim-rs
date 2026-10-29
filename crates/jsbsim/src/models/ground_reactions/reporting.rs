// SPDX-License-Identifier: LGPL-2.1-or-later
// FGLGear: Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/models/FGLGear.cpp (InitializeReporting,
// ResetReporting, ReportTakeoffOrLanding, Report, CrashDetect).
//
//! Takeoff and landing reports and crash detection: they log, and change
//! nothing the simulation reads.

use super::Inputs;
use super::contact::Contact;
use super::frame::FrameContext;
use crate::math::E_Z;
use crate::units::{FTTOM, rankine_to_celsius};

impl Contact {
    /// `InitializeReporting`: the first contact's figures, and the start of
    /// a takeoff run.
    pub(super) fn initialize_reporting(&mut self, input: &Inputs) {
        let r = &mut self.reporting;
        if !r.first_contact {
            r.first_contact = true;
            r.sink_rate = self.state.compress_speed;
            r.ground_speed = input.vground;
            r.takeoff_reported = false;
        }
        let [_, left, right, ..] = input.brake_pos;
        if input.vground > 0.1
            && left == 0.0
            && right == 0.0
            && (input.takeoff_throttle && !r.started_ground_run)
        {
            r.takeoff_distance = 0.0;
            r.takeoff_distance_50ft = 0.0;
            r.started_ground_run = true;
        }
    }

    /// `ResetReporting`: once the vehicle is well clear of the ground.
    pub(super) fn reset_reporting(&mut self, input: &Inputs) {
        if input.distance_agl > 200.0 {
            let r = &mut self.reporting;
            r.first_contact = false;
            r.started_ground_run = false;
            r.landing_reported = false;
            r.takeoff_reported = true;
            r.landing_distance = 0.0;
            r.max_strut_force = 0.0;
            r.max_strut_travel = 0.0;
        }
    }

    /// `ReportTakeoffOrLanding`.
    pub(super) fn report_takeoff_or_landing(&mut self, input: &Inputs, sim_time: f64) {
        let r = &mut self.reporting;
        let step = input.vground * input.total_delta_t;
        if r.first_contact {
            r.landing_distance += step;
        }
        if r.started_ground_run {
            r.takeoff_distance_50ft += step;
            if self.state.wow {
                r.takeoff_distance += step;
            }
        }
        if r.enabled && input.vground <= 0.05 && !r.landing_reported && input.wow {
            self.report_landing(sim_time);
        }
        let r = &self.reporting;
        if r.enabled
            && !r.takeoff_reported
            && (input.distance_agl - self.state.local_gear[E_Z]) > 50.0
            && !input.wow
        {
            self.report_takeoff(input, sim_time);
        }
        let s = &self.state;
        if s.last_wow != s.wow {
            tracing::info!(
                "GEAR_CONTACT: {sim_time:.6} seconds: {} {}",
                self.name,
                u8::from(s.wow)
            );
        }
    }

    /// `Report(erLand)`.
    fn report_landing(&mut self, sim_time: f64) {
        let r = &mut self.reporting;
        if r.takeoff_distance.abs() < 0.001 {
            return;
        }
        tracing::info!(
            "Touchdown report for {} (WOW at time: {sim_time:.6} seconds)\n  Sink rate at contact:  {} fps,    {} mps\n  Contact ground speed:  {} knots,  {} mps\n  Maximum contact force: {} lbs,    {} Newtons\n  Maximum strut travel:  {} inches, {} cm\n  Distance traveled:     {} ft,     {} meters",
            self.name,
            r.sink_rate,
            r.sink_rate * 0.3048,
            r.ground_speed * 0.5925,
            r.ground_speed * 0.3048,
            r.max_strut_force,
            r.max_strut_force * 4.448,
            r.max_strut_travel * 12.0,
            r.max_strut_travel * 30.48,
            r.landing_distance,
            r.landing_distance * 0.3048,
        );
        r.landing_reported = true;
    }

    /// `Report(erTakeoff)`.
    fn report_takeoff(&mut self, input: &Inputs, sim_time: f64) {
        let r = &mut self.reporting;
        if r.takeoff_distance.abs() < 0.001 {
            return;
        }
        tracing::info!(
            "Takeoff report for {} (Liftoff at time: {sim_time:.6} seconds)\n  Distance traveled:                {} ft,     {} meters\n  Distance traveled (over 50'):     {} ft,     {} meters\n  [Altitude (ASL): {} ft. / {} m  | Temperature: {} F / {} C]\n  [Velocity (KCAS): {}]",
            self.name,
            r.takeoff_distance,
            r.takeoff_distance * 0.3048,
            r.takeoff_distance_50ft,
            r.takeoff_distance_50ft * 0.3048,
            input.distance_asl,
            input.distance_asl * FTTOM,
            input.temperature - 459.67,
            rankine_to_celsius(input.temperature),
            input.vcalibrated_kts,
        );
        r.takeoff_reported = true;
    }

    /// `CrashDetect`: out-of-bounds figures are logged; JSBSim does not
    /// suspend the integration over them.
    pub(super) fn crash_detect(&self, frame: &FrameContext) {
        let crashed = self.state.compress_length > 500.0
            || self.force.normal_force.magnitude() > 100_000_000.0
            || self.force.moment.magnitude() > 5_000_000_000.0
            || self.reporting.sink_rate > 1.4666 * 30.0;
        if crashed && !frame.integration_suspended {
            tracing::info!(
                "*CRASH DETECTED* {:.6} seconds: {}",
                frame.sim_time,
                self.name
            );
        }
    }
}
