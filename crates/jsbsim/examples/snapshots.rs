// SPDX-License-Identifier: LGPL-2.1-or-later
//! Rewinds, replays and copies a simulation with `Snapshot`.
//!
//! `cargo run -p jsbsim --example snapshots`

use jsbsim::snapshot::{FingerprintPart, SnapshotError};
use jsbsim::{Error, Fdm, MemoryResolver, Snapshot};

/// A box dropped from 1000 ft; `drag` sets its drag coefficient.
fn dropped_box(drag: f64) -> Result<Fdm, Error> {
    let aircraft = format!(
        r#"<fdm_config name="box" version="2.0">
      <metrics> <wingarea unit="FT2"> 1 </wingarea> <wingspan unit="FT"> 1 </wingspan>
        <chord unit="FT"> 1 </chord> </metrics>
      <mass_balance> <ixx> 10 </ixx> <iyy> 10 </iyy> <izz> 10 </izz>
        <emptywt> 100 </emptywt> <location name="CG"> <x>0</x> <y>0</y> <z>0</z> </location>
      </mass_balance>
      <ground_reactions> <contact type="STRUCTURE" name="BOTTOM">
        <location> <x>0</x> <y>0</y> <z>-10</z> </location>
        <spring_coeff> 1000 </spring_coeff> <damping_coeff> 100 </damping_coeff>
      </contact> </ground_reactions>
      <aerodynamics> <axis name="DRAG"> <function name="aero/drag">
        <product> <property>aero/qbar-psf</property> <value>{drag}</value> </product>
      </function> </axis> </aerodynamics>
    </fdm_config>"#
    );
    let mut files = MemoryResolver::new();
    files.insert("aircraft/box/box.xml", aircraft);
    let mut fdm = Fdm::new()?;
    fdm.load_model(&files, "aircraft", "box")?;
    fdm.set_property("ic/h-sl-ft", 1000.0)?;
    fdm.run_ic()?;
    Ok(fdm)
}

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().try_init().ok();
    let mut fdm = dropped_box(1.0)?;

    // ANCHOR: rewind
    for _ in 0..120 {
        fdm.run()?;
    }
    // Keep one Snapshot and take into it again and again: after the
    // first, taking and restoring allocate nothing.
    let mut saved = Snapshot::new();
    fdm.snapshot_into(&mut saved)?;
    let then = (fdm.sim_time(), fdm.get_property("position/h-sl-ft")?);

    for _ in 0..240 {
        fdm.run()?;
    }
    fdm.restore(&saved)?;
    assert_eq!(
        (fdm.sim_time(), fdm.get_property("position/h-sl-ft")?),
        then
    );
    // ANCHOR_END: rewind

    // ANCHOR: replay
    // The same inputs from the same state give the same bits.
    let mut first = Vec::new();
    for _ in 0..60 {
        fdm.run()?;
        first.push(fdm.get_property("position/h-sl-ft")?.to_bits());
    }
    fdm.restore(&saved)?;
    for bits in first {
        fdm.run()?;
        assert_eq!(fdm.get_property("position/h-sl-ft")?.to_bits(), bits);
    }
    // ANCHOR_END: replay

    // ANCHOR: transfer
    // The bytes are the same on every target: store them, or send them to
    // an executive elsewhere that loaded the same files.
    let bytes: Vec<u8> = saved.as_bytes().to_vec();
    let mut twin = dropped_box(1.0)?;
    twin.restore(&Snapshot::from_bytes(&bytes)?)?;
    assert_eq!(twin.sim_time(), then.0);
    // ANCHOR_END: transfer

    // ANCHOR: mismatch
    // A different model refuses the snapshot and is left as it was.
    let mut other = dropped_box(2.0)?;
    let refused = other.restore(&saved);
    if let Err(Error::Snapshot(SnapshotError::FingerprintMismatch { part })) = &refused {
        tracing::info!("refused: the {part} differ");
    }
    assert_eq!(
        refused,
        Err(Error::Snapshot(SnapshotError::FingerprintMismatch {
            part: FingerprintPart::Documents
        }))
    );
    assert_eq!(other.sim_time(), 0.0);
    // ANCHOR_END: mismatch
    Ok(())
}

#[test]
fn runs() -> Result<(), Error> {
    main()
}
