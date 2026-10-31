// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

#[test]
fn construction_binds_model_and_exec_properties() {
    let mut fdm = Fdm::new().unwrap();
    for path in [
        "simulation/models/FGPropagate/enabled",
        "position/h-sl-ft",
        "accelerations/udot-ft_sec2",
        "metrics/Sw-sqft",
        "simulation/sim-time-sec",
        "forces/hold-down",
    ] {
        assert!(fdm.node(path).unwrap().is_some(), "{path}");
    }
    assert_eq!(
        fdm.get_property("simulation/models/FGPropagate/enabled")
            .unwrap(),
        1.0
    );
    assert_eq!(
        fdm.get_property("simulation/integrator/position/translational")
            .unwrap(),
        4.0
    );
}

#[test]
fn getters_are_live_and_read_only_nodes_reject_writes() {
    let mut fdm = Fdm::new().unwrap();
    assert!(fdm.set_property("metrics/Sw-sqft", 174.0).unwrap());
    assert_eq!(fdm.get_property("metrics/Sw-sqft").unwrap(), 174.0);
    assert!(
        !fdm.set_property("metrics/bw-ft", 1.0).unwrap(),
        "getter-only"
    );
    assert!(!fdm.set_property("simulation/sim-time-sec", 1.0).unwrap());
}

#[test]
fn time_advances_only_when_not_holding() {
    let mut fdm = Fdm::new().unwrap();
    // No aircraft is loaded, so the aerodynamics have no axis system and
    // would refuse to run, as in JSBSim.
    fdm.set_property("simulation/models/FGAerodynamics/enabled", 0.0)
        .unwrap();
    fdm.set_dt(0.5);
    fdm.run().unwrap();
    assert_eq!(fdm.get_property("simulation/sim-time-sec").unwrap(), 0.5);
    fdm.set_property("simulation/pause", 1.0).unwrap();
    fdm.run().unwrap();
    assert_eq!(fdm.get_property("simulation/frame").unwrap(), 1.0);
    fdm.set_property("simulation/terminate", 1.0).unwrap();
    assert!(!fdm.run().unwrap());
}

#[test]
fn model_rates_skip_frames_like_fgmodel() {
    let mut base = crate::models::base::ModelBase::default();
    base.rate = 3;
    let runs: Vec<bool> = (0..7).map(|_| !base.skip()).collect();
    assert_eq!(runs, [true, false, false, true, false, false, true]);
}

#[test]
fn unloaded_aerodynamics_refuse_to_run() {
    let mut fdm = Fdm::new().unwrap();
    assert!(matches!(fdm.run(), Err(Error::Model { .. })));
}

/// `FGAtmosphere::Calculate` looks an override node up on every call
/// until it exists, so one created between frames rules the next frame.
#[test]
fn an_override_created_between_frames_takes_effect_in_the_next_frame() {
    let mut fdm = Fdm::new().unwrap();
    fdm.set_property("simulation/models/FGAerodynamics/enabled", 0.0)
        .unwrap();
    fdm.run().unwrap();
    let modeled = fdm.get_property("atmosphere/T-R").unwrap();
    assert_ne!(modeled, 400.0);
    fdm.set_property("atmosphere/override/temperature", 400.0)
        .unwrap();
    assert_eq!(fdm.get_property("atmosphere/T-R").unwrap(), modeled);
    fdm.run().unwrap();
    assert_eq!(fdm.get_property("atmosphere/T-R").unwrap(), 400.0);
    fdm.set_property("atmosphere/override/temperature", 450.0)
        .unwrap();
    fdm.run().unwrap();
    assert_eq!(fdm.get_property("atmosphere/T-R").unwrap(), 450.0);
}

#[test]
fn takeoff_throttle_follows_the_first_engine_above_ninety_percent() {
    let mut fdm = Fdm::new().unwrap();
    fdm.models.load_inputs(super::order::Model::GroundReactions);
    assert!(!fdm.models.ground_reactions.input.takeoff_throttle);
    fdm.models.fcs.throttles.add();
    fdm.models.fcs.throttles.throttle_pos[0] = 0.9;
    fdm.models.load_inputs(super::order::Model::GroundReactions);
    assert!(!fdm.models.ground_reactions.input.takeoff_throttle);
    fdm.models.fcs.throttles.throttle_pos[0] = 0.95;
    fdm.models.load_inputs(super::order::Model::GroundReactions);
    assert!(fdm.models.ground_reactions.input.takeoff_throttle);
}

/// A reset set by a script event resets the events at once (`ResetEvents`
/// inside `RunScript`): the event that reset sees it, and every event fires
/// again in the next run.
#[test]
fn a_reset_from_a_script_event_rearms_the_events() {
    let aircraft = r#"<fdm_config name="box">
  <metrics/>
  <mass_balance>
    <ixx>10</ixx><iyy>10</iyy><izz>10</izz><emptywt>100</emptywt>
    <location name="CG" unit="IN"><x>0</x><y>0</y><z>0</z></location>
  </mass_balance>
  <ground_reactions/>
</fdm_config>"#;
    let ic = r#"<initialize><altitude unit="FT">10000</altitude></initialize>"#;
    let script = r#"<runscript name="reset">
  <use aircraft="box" initialize="ic"/>
  <run start="0.0" end="1" dt="0.01">
    <property value="0" persistent="true"> test/runs </property>
    <property value="0"> test/fired </property>
    <property value="0"> test/after </property>
    <event name="fire">
      <condition> simulation/sim-time-sec ge 0.05 </condition>
      <set name="test/fired" type="FG_DELTA" value="1"/>
    </event>
    <event name="reset">
      <condition logic="AND">
        simulation/sim-time-sec ge 0.1
        test/runs lt 2
      </condition>
      <set name="simulation/reset" value="1"/>
      <set name="test/runs" type="FG_DELTA" value="1"/>
      <set name="test/after" value="1"/>
    </event>
  </run>
</runscript>"#;
    let mut r = crate::MemoryResolver::new();
    r.insert("aircraft/box/box.xml", aircraft);
    r.insert("aircraft/box/ic.xml", ic);
    r.insert("scripts/reset.xml", script);
    let mut fdm = Fdm::new().unwrap();
    fdm.set_property("simulation/models/FGAerodynamics/enabled", 0.0)
        .unwrap();
    fdm.load_script(&r, "scripts/reset.xml").unwrap();
    fdm.run_ic().unwrap();
    let mut resets = 0;
    let mut last = 0.0;
    for _ in 0..40 {
        fdm.run().unwrap();
        let t = fdm.models.exec.sim_time;
        if t < last {
            resets += 1;
            // The reset restored the declared property before the event's
            // later sets wrote theirs.
            assert_eq!(fdm.get_property("test/fired").unwrap(), 0.0);
            assert_eq!(fdm.get_property("test/after").unwrap(), 1.0);
        }
        last = t;
    }
    assert_eq!(resets, 2);
    assert_eq!(fdm.get_property("test/runs").unwrap(), 2.0);
    // The third run fired the first event again.
    assert_eq!(fdm.get_property("test/fired").unwrap(), 1.0);
}

/// `load_model_in_dir` reads `dir/model.xml` and resolves the model's
/// own files against `dir`, where `load_model` looks in `dir/model/`.
#[test]
fn a_model_kept_directly_in_its_directory_loads_in_dir() {
    let aircraft = r#"<fdm_config name="flat">
  <metrics/>
  <mass_balance file="Mass.xml"/>
  <ground_reactions/>
</fdm_config>"#;
    let mass = r#"<mass_balance>
  <ixx>10</ixx><iyy>10</iyy><izz>10</izz><emptywt>100</emptywt>
  <location name="CG" unit="IN"><x>0</x><y>0</y><z>0</z></location>
</mass_balance>"#;
    let mut r = crate::MemoryResolver::new();
    r.insert("models/Flat/flat.xml", aircraft);
    r.insert("models/Flat/Mass.xml", mass);
    let missing = Fdm::new().unwrap().load_model(&r, "models/Flat", "flat");
    assert!(
        matches!(&missing, Err(Error::ResourceNotFound { path }) if path == "models/Flat/flat/flat.xml"),
        "{missing:?}"
    );
    let mut fdm = Fdm::new().unwrap();
    fdm.load_model_in_dir(&r, "models/Flat", "flat").unwrap();
    assert_eq!(fdm.get_property("inertia/empty-weight-lbs").unwrap(), 100.0);
}
