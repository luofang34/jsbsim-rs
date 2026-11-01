// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use crate::test_log::messages as logged;
use crate::units::RADTODEG;

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

/// The axis row for control `name` after `  Trim Results:`, found by the
/// padded name column `AxisReport` writes, so no other line can match.
fn result_line<'a>(lines: &'a [String], name: &str) -> &'a str {
    let results = lines
        .iter()
        .position(|l| l == "  Trim Results:")
        .unwrap_or_else(|| panic!("no Trim Results in {lines:?}"));
    let column = format!("  {name:<20}: ");
    lines[results..]
        .iter()
        .find(|l| l.starts_with(&column))
        .unwrap_or_else(|| panic!("no {name} row in {lines:?}"))
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
    assert_eq!(fdm.trims_run(), 1);
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
    // Naming every field makes a new one, such as a display factor, fail to
    // compile here: the display factor belongs to the control, and the
    // report holds only values, alpha in radians.
    let AxisReport {
        state,
        control,
        control_value,
        state_value,
        tolerance,
        sub_iterations: _,
        successful: _,
        stability: _,
        run_count,
        avg_stability: _,
    } = report.axes[0];
    assert_eq!((state, control), (State::Wdot, Control::Alpha));
    assert!(state_value.abs() <= tolerance, "{state_value}");
    assert!(run_count > 0);
    assert!((0.05..0.2).contains(&control_value), "{control_value}");
    let line = result_line(&lines, "Angle of Attack");
    let degrees = format!(
        "  {:<20}: {:<6.2} wdot : ",
        "Angle of Attack",
        control_value * RADTODEG
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
