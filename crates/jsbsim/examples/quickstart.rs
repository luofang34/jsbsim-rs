// SPDX-License-Identifier: LGPL-2.1-or-later
//! Drops a box from 1000 ft: the smallest aircraft that loads, held in
//! memory, so it needs no JSBSim data tree.
//!
//! `cargo run -p jsbsim --example quickstart`

use jsbsim::{Fdm, MemoryResolver};

// ANCHOR: aircraft
/// A rigid box with a mass, inertia, one contact point and no aerodynamics.
const BOX: &str = r#"<fdm_config name="box" version="2.0">
  <metrics>
    <wingarea unit="FT2"> 1 </wingarea> <wingspan unit="FT"> 1 </wingspan>
    <chord unit="FT"> 1 </chord>
  </metrics>
  <mass_balance>
    <ixx> 10 </ixx> <iyy> 10 </iyy> <izz> 10 </izz> <emptywt> 100 </emptywt>
    <location name="CG"> <x>0</x> <y>0</y> <z>0</z> </location>
  </mass_balance>
  <ground_reactions>
    <contact type="STRUCTURE" name="BOTTOM">
      <location> <x>0</x> <y>0</y> <z>-10</z> </location>
      <spring_coeff> 1000 </spring_coeff> <damping_coeff> 100 </damping_coeff>
    </contact>
  </ground_reactions>
  <aerodynamics> <axis name="DRAG"/> </aerodynamics>
</fdm_config>"#;
// ANCHOR_END: aircraft

fn main() -> Result<(), jsbsim::Error> {
    tracing_subscriber::fmt().try_init().ok();

    // ANCHOR: run
    // The files the simulation may read, by path.
    let mut files = MemoryResolver::new();
    files.insert("aircraft/box/box.xml", BOX);

    // One executive per aircraft: load it, set and apply initial
    // conditions, then step.
    let mut fdm = Fdm::new()?;
    fdm.load_model(&files, "aircraft", "box")?;
    fdm.set_property("ic/h-sl-ft", 1000.0)?;
    fdm.run_ic()?;

    while fdm.sim_time() < 5.0 {
        fdm.run()?;
    }
    let altitude = fdm.get_property("position/h-sl-ft")?;
    let sink_rate = fdm.get_property("velocities/h-dot-fps")?;
    tracing::info!(
        "after {:.2} s: {altitude:.1} ft, {sink_rate:.1} ft/s",
        fdm.sim_time()
    );
    // ANCHOR_END: run

    // Five seconds of free fall: about 400 ft down, at about 160 ft/s.
    assert!((550.0..650.0).contains(&altitude), "altitude {altitude}");
    assert!(
        (-170.0..-150.0).contains(&sink_rate),
        "sink rate {sink_rate}"
    );
    Ok(())
}

#[test]
fn runs() -> Result<(), jsbsim::Error> {
    main()
}
