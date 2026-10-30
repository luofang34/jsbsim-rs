// SPDX-License-Identifier: LGPL-2.1-or-later
use crate::math::{Axis, E_PHI, E_PSI, E_THT};
use crate::{Fdm, MemoryResolver};

const BOX: &str = r#"<fdm_config name="box" version="2.0">
  <metrics> <wingarea unit="FT2"> 1 </wingarea> <wingspan unit="FT"> 1 </wingspan>
    <chord unit="FT"> 1 </chord> </metrics>
  <mass_balance> <ixx> 10 </ixx> <iyy> 10 </iyy> <izz> 10 </izz>
    <emptywt> 100 </emptywt> <location name="CG"> <x>0</x> <y>0</y> <z>0</z> </location>
  </mass_balance>
  <ground_reactions/>
  <aerodynamics> <axis name="DRAG"/> </aerodynamics>
  <system name="mag">
    <channel name="mag">
      <magnetometer name="test/mag-x">
        <location unit="IN"> <x>0</x> <y>0</y> <z>0</z> </location>
        <axis>X</axis>
      </magnetometer>
    </channel>
  </system>
</fdm_config>"#;

fn loaded(wall_clock: Option<i64>) -> Fdm {
    let mut files = MemoryResolver::new();
    files.insert("aircraft/box/box.xml", BOX);
    let mut fdm = Fdm::new().unwrap();
    if let Some(seconds) = wall_clock {
        fdm.set_wall_clock(seconds);
    }
    fdm.load_model(&files, "aircraft", "box").unwrap();
    fdm
}

fn start(fdm: &mut Fdm, theta_deg: f64) {
    for (name, value) in [
        ("ic/h-sl-ft", 10_000.0),
        ("ic/lat-geod-deg", 45.0),
        ("ic/long-gc-deg", -100.0),
        ("ic/phi-deg", 10.0),
        ("ic/theta-deg", theta_deg),
        ("ic/psi-deg", 30.0),
    ] {
        fdm.set_property(name, value).unwrap();
    }
    fdm.run_ic().unwrap();
}

#[test]
fn clock_getters_match_the_simulation_properties() {
    let mut fdm = loaded(None);
    fdm.set_dt(0.01);
    start(&mut fdm, 5.0);
    for _ in 0..7 {
        fdm.run().unwrap();
    }
    assert_eq!(fdm.dt(), 0.01);
    assert_eq!(
        fdm.sim_time(),
        fdm.get_property("simulation/sim-time-sec").unwrap()
    );
    assert!(fdm.sim_time() > 0.0);
}

#[test]
fn pose_getters_match_the_bound_position_and_attitude() {
    let mut fdm = loaded(None);
    start(&mut fdm, 5.0);
    fdm.run().unwrap();
    let euler = fdm.quaternion().euler();
    for (axis, property) in [
        (E_PHI, "attitude/phi-rad"),
        (E_THT, "attitude/theta-rad"),
        (E_PSI, "attitude/psi-rad"),
    ] {
        assert_eq!(
            euler[axis],
            fdm.get_property(property).unwrap(),
            "{property}"
        );
    }
    let location = *fdm.location();
    assert_eq!(
        location.geod_latitude_deg(),
        fdm.get_property("position/lat-geod-deg").unwrap()
    );
    assert_eq!(
        location.longitude_deg(),
        fdm.get_property("position/long-gc-deg").unwrap()
    );
    // ECEF-to-body is local-to-body after ECEF-to-local.
    let ecef = fdm.quaternion_ecef().t();
    let composed = fdm.quaternion().t() * location.tec2l();
    for row in [Axis::X, Axis::Y, Axis::Z] {
        for column in [Axis::X, Axis::Y, Axis::Z] {
            let d = ecef[(row, column)] - composed[(row, column)];
            assert!(d.abs() < 1e-12, "({row:?}, {column:?}): {d}");
        }
    }
    assert!((fdm.quaternion_eci().magnitude() - 1.0).abs() < 1e-12);
}

#[test]
fn the_attitude_quaternion_holds_a_vertical_attitude() {
    let mut fdm = loaded(None);
    start(&mut fdm, 90.0);
    fdm.run().unwrap();
    let q = fdm.quaternion();
    assert!(q.to_array().iter().all(|c| c.is_finite()));
    assert!((q.magnitude() - 1.0).abs() < 1e-12);
    // The body x axis, the first row of local-to-body, points up (-z NED).
    let t = q.t();
    assert!((t[(Axis::X, Axis::Z)] + 1.0).abs() < 1e-9);
}

#[test]
fn the_wall_clock_dates_the_magnetometer_field_model() {
    let field = |clock: Option<i64>| {
        let mut fdm = loaded(clock);
        start(&mut fdm, 0.0);
        fdm.run().unwrap();
        fdm.get_property("test/mag-x").unwrap()
    };
    // 1 January 2005, the WMM2005 epoch, and 1 January 2025.
    let epoch = field(Some(1_104_537_600));
    let later = field(Some(1_735_689_600));
    assert_ne!(epoch, later);
    assert_eq!(field(None), field(Some(0)));
}
