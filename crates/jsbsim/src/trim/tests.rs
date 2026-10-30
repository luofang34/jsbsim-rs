// SPDX-License-Identifier: LGPL-2.1-or-later
use super::axis::DEFAULT_TOLERANCE;
use super::*;
use crate::math::{E_X, E_Z};
use crate::{Error, MemoryResolver};

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
