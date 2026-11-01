// SPDX-License-Identifier: LGPL-2.1-or-later
use super::axis::DEFAULT_TOLERANCE;
use super::*;
use crate::math::{E_X, E_Z};
use crate::units::RADTODEG;
use crate::{Error, MemoryResolver};
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

/// A tripod of three bogeys under a point mass, without aerodynamics, as
/// JSBSim's tests/tripod.xml.
const TRIPOD: &str = r#"<fdm_config name="tripod" version="2.0">
    <metrics>
        <wingarea unit="FT2"> 1 </wingarea>
        <wingspan unit="FT"> 1 </wingspan>
        <chord unit="FT"> 1 </chord>
    </metrics>
    <mass_balance>
        <ixx unit="SLUG*FT2"> 100 </ixx>
        <iyy unit="SLUG*FT2"> 100 </iyy>
        <izz unit="SLUG*FT2"> 100 </izz>
        <emptywt unit="LBS"> 100 </emptywt>
        <location name="CG" unit="IN"> <x> 0 </x> <y> 0 </y> <z> 0 </z> </location>
    </mass_balance>
    <ground_reactions>
        <contact type="BOGEY" name="A">
            <location unit="IN"> <x> 20 </x> <y> 0 </y> <z> -10 </z> </location>
            <static_friction> 0.1 </static_friction>
            <dynamic_friction> 0.1 </dynamic_friction>
            <spring_coeff unit="LBS/FT"> 10000 </spring_coeff>
            <damping_coeff unit="LBS/FT/SEC"> 1000 </damping_coeff>
        </contact>
        <contact type="BOGEY" name="B">
            <location unit="IN"> <x> -10 </x> <y> 17.32 </y> <z> -10 </z> </location>
            <static_friction> 0.1 </static_friction>
            <dynamic_friction> 0.1 </dynamic_friction>
            <spring_coeff unit="LBS/FT"> 10000 </spring_coeff>
            <damping_coeff unit="LBS/FT/SEC"> 1000 </damping_coeff>
        </contact>
        <contact type="BOGEY" name="C">
            <location unit="IN"> <x> -10 </x> <y> -17.32 </y> <z> -10 </z> </location>
            <static_friction> 0.1 </static_friction>
            <dynamic_friction> 0.1 </dynamic_friction>
            <spring_coeff unit="LBS/FT"> 10000 </spring_coeff>
            <damping_coeff unit="LBS/FT/SEC"> 1000 </damping_coeff>
        </contact>
    </ground_reactions>
</fdm_config>"#;

fn tripod() -> Fdm {
    let mut fdm = Fdm::new().unwrap();
    fdm.set_property("simulation/models/FGAerodynamics/enabled", 0.0)
        .unwrap();
    let mut r = MemoryResolver::new();
    r.insert("aircraft/tripod/tripod.xml", TRIPOD);
    fdm.load_model(&r, "aircraft", "tripod").unwrap();
    fdm
}

/// The tripod clear of the ground with lift that grows with alpha, which a
/// custom wdot axis trims with alpha.
fn lifting_tripod() -> Fdm {
    const LIFT: &str = r#"<aerodynamics>
        <axis name="LIFT">
            <function name="aero/force/lift">
                <product>
                    <property>aero/qbar-psf</property>
                    <property>metrics/Sw-sqft</property>
                    <property>aero/alpha-rad</property>
                    <value>20</value>
                </product>
            </function>
        </axis>
    </aerodynamics>
</fdm_config>"#;
    let mut fdm = Fdm::new().unwrap();
    let mut r = MemoryResolver::new();
    let text = TRIPOD.replace("</fdm_config>", LIFT);
    r.insert("aircraft/tripod/tripod.xml", text.as_str());
    fdm.load_model(&r, "aircraft", "tripod").unwrap();
    fdm.set_property("ic/h-sl-ft", 1000.0).unwrap();
    fdm.set_property("ic/vt-fps", 200.0).unwrap();
    fdm.run_ic().unwrap();
    fdm
}

/// Keeps the message of every event logged on its thread.
struct Capture(Arc<Mutex<Vec<String>>>);

struct Message(String);

impl Visit for Message {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }
}

impl Subscriber for Capture {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        let mut message = Message(String::new());
        event.record(&mut message);
        self.0.lock().unwrap().push(message.0);
    }
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

/// What `run` logs on this thread, and its result.
fn logged<T>(run: impl FnOnce() -> T) -> (Vec<String>, T) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let result = tracing::subscriber::with_default(Capture(Arc::clone(&lines)), run);
    let lines = lines.lock().unwrap().clone();
    (lines, result)
}

/// The line after `  Trim Results:` that starts with `name`.
fn result_line<'a>(lines: &'a [String], name: &str) -> &'a str {
    let results = lines
        .iter()
        .position(|l| l == "  Trim Results:")
        .unwrap_or_else(|| panic!("no Trim Results in {lines:?}"));
    lines[results..]
        .iter()
        .find(|l| l.trim_start().starts_with(name))
        .unwrap_or_else(|| panic!("no {name} in {lines:?}"))
}

#[test]
fn modes_have_jsbsims_values() {
    for v in 0..=6 {
        assert_eq!(TrimMode::from_i32(v).map(|m| m as i32), Some(v));
    }
    assert_eq!(TrimMode::from_i32(-1), None);
    assert_eq!(TrimMode::from_i32(7), None);
}

#[test]
fn a_mode_out_of_range_is_an_illegal_trim() {
    let mut fdm = tripod();
    fdm.run_ic().unwrap();
    for mode in [-1.0, 7.0] {
        let err = fdm.set_property("simulation/do_simple_trim", mode);
        assert_eq!(
            err,
            Err(Error::TrimFailed {
                message: "Illegal trimming mode!".to_owned()
            })
        );
    }
    assert_eq!(fdm.trims_run(), 0);
    assert_eq!(fdm.get_property("simulation/trim-completed").unwrap(), 0.0);
}

#[test]
fn every_typed_mode_trims_as_simulation_do_simple_trim_does() {
    // The ground trim needs the tripod on its springs. The other modes fail
    // without aerodynamics, and clear of the ground they reach that failure
    // in far fewer iterations.
    let started = |mode| {
        let mut fdm = tripod();
        if mode == TrimMode::Ground {
            fdm.set_property("ic/h-sl-ft", 0.1).unwrap();
            fdm.set_property("ic/lat-gc-deg", 90.0).unwrap();
        } else {
            fdm.set_property("ic/h-sl-ft", 1000.0).unwrap();
        }
        fdm.run_ic().unwrap();
        fdm
    };
    for v in 0..=6 {
        let mode = TrimMode::from_i32(v).unwrap();
        let mut typed = started(mode);
        let typed_result = typed.do_trim(mode);
        let mut raw = started(mode);
        let raw_result = raw
            .set_property("simulation/do_simple_trim", f64::from(v))
            .map(drop);
        assert_eq!(typed_result, raw_result, "{mode:?}");
        // Debug text, as aero-less modes leave NaN states that `==` rejects.
        assert_eq!(
            format!("{:?}", typed.last_trim()),
            format!("{:?}", raw.last_trim())
        );
        assert_eq!(typed.last_trim().map(|r| r.mode), Some(mode));
        let h = "position/h-sl-ft";
        assert_eq!(
            typed.get_property(h).unwrap().to_bits(),
            raw.get_property(h).unwrap().to_bits(),
            "{mode:?}"
        );
    }
}

#[test]
fn custom_and_none_trims_have_no_axes_and_succeed_without_moving_time() {
    for mode in [TrimMode::Custom, TrimMode::None] {
        let mut fdm = tripod();
        fdm.set_property("ic/h-sl-ft", 1000.0).unwrap();
        fdm.run_ic().unwrap();
        fdm.run().unwrap();
        let (t, dt) = (fdm.models.exec.sim_time, fdm.models.exec.dt);
        fdm.set_property("simulation/do_simple_trim", f64::from(mode as i32))
            .unwrap();
        let report = fdm.last_trim().unwrap();
        assert!(report.success && report.axes.is_empty());
        assert_eq!(report.total_iterations, 1);
        assert_eq!(fdm.models.exec.sim_time, t);
        assert_eq!(fdm.models.exec.dt, dt);
        assert!(!fdm.models.exec.trim_status);
        assert_eq!(fdm.get_property("simulation/trim-completed").unwrap(), 1.0);
    }
}

#[test]
fn the_ground_trim_rests_the_tripod_on_its_three_springs() {
    let mut fdm = tripod();
    fdm.set_property("ic/h-sl-ft", 0.1).unwrap();
    fdm.set_property("ic/lat-gc-deg", 90.0).unwrap();
    fdm.run_ic().unwrap();
    let h = fdm.get_property("ic/h-sl-ft").unwrap();
    fdm.do_trim(TrimMode::Ground).unwrap();
    let report = fdm.last_trim().unwrap().clone();
    assert!(report.success);
    let states: Vec<_> = report.axes.iter().map(|a| (a.state, a.control)).collect();
    assert_eq!(
        states,
        [
            (State::Wdot, Control::AltAgl),
            (State::Qdot, Control::Theta),
            (State::Pdot, Control::Phi)
        ]
    );
    for a in &report.axes {
        assert!(a.state_value.abs() <= a.tolerance, "{a:?}");
        assert!(a.run_count > 0);
    }
    let weight = fdm.get_property("forces/fbz-weight-lbs").unwrap();
    let gear = fdm.get_property("forces/fbz-gear-lbs").unwrap();
    assert!((gear / weight + 1.0).abs() < 1e-3, "{gear} vs {weight}");
    assert!(
        fdm.models
            .ground_reactions
            .contacts
            .iter()
            .all(|c| c.reporting.enabled)
    );
    // The executive's own initial conditions are not the trimmed ones.
    assert_eq!(fdm.get_property("ic/h-sl-ft").unwrap(), h);
    assert_ne!(fdm.get_property("position/h-sl-ft").unwrap(), h);
}

#[test]
fn a_failed_trim_puts_the_commands_back() {
    let mut fdm = tripod();
    fdm.set_property("ic/h-sl-ft", 1000.0).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("fcs/elevator-cmd-norm", 0.25).unwrap();
    fdm.set_property("fcs/pitch-trim-cmd-norm", -0.5).unwrap();
    // Nothing on the tripod moves wdot with alpha or qdot with the trim.
    let err = fdm.set_property("simulation/do_simple_trim", 0.0);
    assert_eq!(
        err,
        Err(Error::TrimFailed {
            message: "Trim Failed".to_owned()
        })
    );
    assert!(!fdm.last_trim().unwrap().success);
    assert_eq!(fdm.models.fcs.de_cmd, 0.25);
    assert_eq!(fdm.models.fcs.ptrim_cmd, -0.5);
    assert_eq!(fdm.get_property("simulation/trim-completed").unwrap(), 0.0);
    assert!(!fdm.models.exec.integration_suspended());
}

#[test]
fn axes_take_their_ranges_from_the_models_and_initial_conditions() {
    let mut fdm = tripod();
    fdm.set_property("ic/theta-deg", 10.0).unwrap();
    let ic = fdm.models.ic.clone();
    let alpha = TrimAxis::new(&fdm, &ic, State::Wdot, Control::Alpha);
    // No CL limits: JSBSim's fallback range.
    assert_eq!(alpha.control_min(), -5.0 * DEGTORAD);
    assert_eq!(alpha.control_max(), 20.0 * DEGTORAD);
    assert_eq!(alpha.control(), 7.5 * DEGTORAD);
    assert_eq!(alpha.solver_eps(), DEFAULT_TOLERANCE / 100.0);
    let theta = TrimAxis::new(&fdm, &ic, State::Qdot, Control::Theta);
    let t = ic.euler_rad(E_THT);
    assert_eq!(theta.control_min(), t - 5.0 * DEGTORAD);
    assert_eq!(theta.tolerance(), DEFAULT_TOLERANCE / 10.0);
    let nlf = TrimAxis::new(&fdm, &ic, State::Nlf, Control::Alpha);
    assert_eq!(nlf.state_target(), 1.0);
    assert_eq!(nlf.tolerance(), 1E-5);
    let throttle = TrimAxis::new(&fdm, &ic, State::Udot, Control::Throttle);
    assert_eq!((throttle.control_min(), throttle.control_max()), (0.0, 1.0));
    assert_eq!(throttle.control(), 0.5);
}

#[test]
fn custom_axes_are_added_once_edited_and_removed() {
    let fdm = tripod();
    let mut trim = Trim::new(&fdm, TrimMode::Longitudinal);
    assert_eq!(trim.axes().len(), 3);
    assert!(!trim.add_state(&fdm, State::Udot, Control::Theta));
    assert!(trim.add_state(&fdm, State::Rdot, Control::Rudder));
    assert!(trim.edit_state(&fdm, State::Udot, Control::Gamma));
    assert_eq!(trim.axes()[1].control_type(), Control::Gamma);
    assert!(trim.remove_state(State::Wdot));
    assert!(!trim.remove_state(State::Wdot));
    let kinds: Vec<_> = trim.axes().iter().map(TrimAxis::state_type).collect();
    assert_eq!(kinds, [State::Udot, State::Qdot, State::Rdot]);
}

#[test]
fn turn_and_pullup_set_the_rates_of_their_load_factor() {
    let mut fdm = tripod();
    fdm.set_property("ic/h-sl-ft", 5000.0).unwrap();
    fdm.set_property("ic/vt-fps", 200.0).unwrap();
    fdm.set_property("ic/phi-deg", 30.0).unwrap();
    fdm.run_ic().unwrap();
    let mut turn = Trim::new(&fdm, TrimMode::Turn);
    turn.setup_turn(&fdm);
    assert_eq!(turn.target_nlf(), 1.0 / libm::cos(30.0 * DEGTORAD));
    turn.update_rates_of(&fdm);
    let pqr = turn.fgic.pqr_radps();
    assert!(pqr[E_Z] > 0.0 && pqr[E_X].abs() < pqr[E_Z]);
    let mut pullup = Trim::new(&fdm, TrimMode::Pullup);
    pullup.set_target_nlf(2.0);
    pullup.setup_pullup(&fdm);
    assert!(pullup.fgic.pqr_radps()[crate::math::E_Q] > 0.0);
}

/// `do_trim_custom` trims the axes it is given, in order, and keeps the
/// first control of a repeated state.
#[test]
fn a_custom_trim_trims_the_axes_it_is_given() {
    let mut fdm = tripod();
    fdm.set_property("ic/h-sl-ft", 0.1).unwrap();
    fdm.set_property("ic/lat-gc-deg", 90.0).unwrap();
    fdm.run_ic().unwrap();
    fdm.do_trim_custom(&[
        (State::Wdot, Control::AltAgl),
        (State::Qdot, Control::Theta),
        (State::Wdot, Control::Throttle),
        (State::Pdot, Control::Phi),
    ])
    .unwrap();
    let report = fdm.last_trim().unwrap();
    assert!(report.success);
    let axes: Vec<_> = report.axes.iter().map(|a| (a.state, a.control)).collect();
    assert_eq!(
        axes,
        [
            (State::Wdot, Control::AltAgl),
            (State::Qdot, Control::Theta),
            (State::Pdot, Control::Phi)
        ]
    );
    for a in &report.axes {
        assert!(a.state_value.abs() <= a.tolerance, "{a:?}");
    }
    assert_eq!(fdm.get_property("simulation/trim-completed").unwrap(), 1.0);
}

/// `Trim Results:` shows alpha as `FGTrimAxis::AxisReport` does, in degrees,
/// while the report keeps it in radians and debug logging changes no count.
#[test]
fn trim_results_log_alpha_in_degrees_and_leave_the_report_as_it_was() {
    let axes = [(State::Wdot, Control::Alpha)];
    let mut quiet = lifting_tripod();
    quiet.do_trim_custom(&axes).unwrap();
    let mut debug = lifting_tripod();
    debug.set_property("simulation/jsbsim-debug", 1.0).unwrap();
    let (lines, result) = logged(|| debug.do_trim_custom(&axes));
    result.unwrap();

    let report = debug.last_trim().unwrap();
    assert_eq!(Some(report), quiet.last_trim());
    let alpha = &report.axes[0];
    assert!(alpha.run_count > 0);
    assert!(
        (0.05..0.2).contains(&alpha.control_value),
        "{}",
        alpha.control_value
    );
    let line = result_line(&lines, "Angle of Attack");
    let degrees = format!(
        "  {:<20}: {:<6.2} wdot : ",
        "Angle of Attack",
        alpha.control_value * RADTODEG
    );
    assert!(line.starts_with(&degrees), "{line:?} vs {degrees:?}");
    assert!(line.ends_with("  Passed"), "{line:?}");
}

/// `AxisReport` converts only the controls whose `FGTrimAxis` constructor
/// sets `control_convert`: pitch and heading stay in radians.
#[test]
fn trim_results_convert_only_the_controls_axis_report_converts() {
    let mut fdm = tripod();
    fdm.set_property("ic/h-sl-ft", 1000.0).unwrap();
    fdm.set_property("ic/theta-deg", 10.0).unwrap();
    fdm.set_property("ic/phi-deg", 20.0).unwrap();
    fdm.set_property("ic/psi-true-deg", 30.0).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("simulation/jsbsim-debug", 1.0).unwrap();
    for (control, name, convert) in [
        (Control::Theta, "Pitch Angle", 1.0),
        (Control::Heading, "Heading", 1.0),
        (Control::Phi, "Roll Angle", RADTODEG),
    ] {
        let (lines, result) = logged(|| fdm.do_trim_custom(&[(State::All, control)]));
        result.unwrap();
        let value = fdm.last_trim().unwrap().axes[0].control_value;
        assert!(value > 0.1, "{name}: {value}");
        let shown = format!("  {name:<20}: {:<6.2} all  : ", value * convert);
        let line = result_line(&lines, name);
        assert!(line.starts_with(&shown), "{line:?} vs {shown:?}");
    }
    let ic = fdm.models.ic.clone();
    for (control, convert) in [
        (Control::Throttle, 1.0),
        (Control::Beta, RADTODEG),
        (Control::Alpha, RADTODEG),
        (Control::Elevator, 1.0),
        (Control::Aileron, 1.0),
        (Control::Rudder, 1.0),
        (Control::AltAgl, 1.0),
        (Control::Gamma, RADTODEG),
        (Control::PitchTrim, 1.0),
        (Control::RollTrim, 1.0),
        (Control::YawTrim, 1.0),
    ] {
        let axis = TrimAxis::new(&fdm, &ic, State::Wdot, control);
        assert_eq!(axis.converts().0, convert, "{control:?}");
    }
}

/// A `tAll` axis reads no state, as `FGTrimAxis::getState` breaks out of its
/// switch: the control stays at the middle of its range, unsolved, after the
/// axis' start and `checkLimits`' three runs of two frames each.
#[test]
fn a_state_all_axis_counts_as_trimmed_and_solves_nothing() {
    let mut fdm = tripod();
    fdm.set_property("ic/h-sl-ft", 1000.0).unwrap();
    fdm.set_property("ic/vt-fps", 200.0).unwrap();
    fdm.run_ic().unwrap();
    fdm.do_trim_custom(&[
        (State::All, Control::Alpha),
        (State::All, Control::Throttle),
    ])
    .unwrap();
    let report = fdm.last_trim().unwrap();
    assert!(report.success);
    assert_eq!(report.total_iterations, 1);
    let [axis] = report.axes.as_slice() else {
        panic!(
            "a repeated state keeps its first control: {:?}",
            report.axes
        );
    };
    assert_eq!((axis.state, axis.control), (State::All, Control::Alpha));
    assert_eq!(axis.control_value, 7.5 * DEGTORAD);
    assert_eq!(axis.state_value, 0.0);
    assert_eq!(axis.tolerance, DEFAULT_TOLERANCE);
    assert_eq!((axis.sub_iterations, axis.successful), (0.0, 1.0));
    assert_eq!((axis.stability, axis.run_count), (2, 8));
    let alpha = fdm.get_property("aero/alpha-deg").unwrap();
    assert!((alpha - 7.5).abs() < 1e-9, "{alpha}");
}

/// A `tAll` axis beside a real one neither blocks nor trims it.
#[test]
fn a_state_all_axis_leaves_the_other_axes_to_trim() {
    let mut fdm = lifting_tripod();
    fdm.do_trim_custom(&[
        (State::All, Control::Elevator),
        (State::Wdot, Control::Alpha),
    ])
    .unwrap();
    let report = fdm.last_trim().unwrap();
    assert!(report.success);
    let wdot = &report.axes[1];
    assert!(wdot.state_value.abs() <= wdot.tolerance, "{wdot:?}");
    assert_eq!(report.axes[0].state_value, 0.0);
}
