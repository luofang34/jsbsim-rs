// SPDX-License-Identifier: LGPL-2.1-or-later
//! A [`NodeId`] names a node of one `Fdm` only, and a snapshot moves between
//! executives without carrying that identity along.

use jsbsim::{Error, Fdm, MemoryResolver, NodeId, Snapshot};

const ALTITUDE: &str = "position/h-sl-ft";
const COMMAND: &str = "fcs/elevator-cmd-norm";

const AIRCRAFT: &str = r#"<fdm_config name="box" version="2.0">
  <metrics> <wingarea unit="FT2"> 1 </wingarea> <wingspan unit="FT"> 1 </wingspan>
    <chord unit="FT"> 1 </chord> </metrics>
  <mass_balance> <ixx> 10 </ixx> <iyy> 10 </iyy> <izz> 10 </izz>
    <emptywt> 100 </emptywt> <location name="CG"> <x>0</x> <y>0</y> <z>0</z> </location>
  </mass_balance>
  <ground_reactions> <contact type="STRUCTURE" name="BOTTOM">
    <location> <x>0</x> <y>0</y> <z>-10</z> </location>
    <spring_coeff> 1000 </spring_coeff> <damping_coeff> 100 </damping_coeff>
  </contact> </ground_reactions>
  <aerodynamics> <axis name="DRAG"/> </aerodynamics> </fdm_config>"#;

/// An executive that has fallen from `altitude` feet for some frames.
#[cfg(test)]
fn falling(altitude: f64) -> Fdm {
    let mut files = MemoryResolver::new();
    files.insert("aircraft/box/box.xml", AIRCRAFT);
    let mut fdm = Fdm::new().unwrap();
    fdm.load_model(&files, "aircraft", "box").unwrap();
    fdm.set_property("ic/h-sl-ft", altitude).unwrap();
    fdm.run_ic().unwrap();
    fly(&mut fdm, 10);
    fdm
}

#[cfg(test)]
fn fly(fdm: &mut Fdm, frames: u32) {
    for _ in 0..frames {
        fdm.run().unwrap();
    }
}

#[cfg(test)]
fn node(fdm: &Fdm, path: &str) -> NodeId {
    fdm.node(path).unwrap().unwrap()
}

#[test]
fn another_fdms_node_is_refused_and_touches_nothing() {
    let (a, mut b) = (falling(1000.0), falling(2000.0));
    let theirs = node(&a, ALTITUDE);
    assert_ne!(theirs, node(&b, ALTITUDE));
    let before = b.get_property(ALTITUDE).unwrap();
    assert_eq!(b.read_node(theirs), Err(Error::ForeignNode));
    assert_eq!(b.write_node(theirs, 5.0), Err(Error::ForeignNode));
    assert_eq!(
        b.write_node(node(&a, COMMAND), 0.5),
        Err(Error::ForeignNode)
    );
    assert_eq!(b.get_property(ALTITUDE).unwrap(), before);
    assert_eq!(b.get_property(COMMAND).unwrap(), 0.0);
}

#[test]
fn a_foreign_node_error_says_so_and_has_no_location() {
    assert_eq!(
        Error::ForeignNode.to_string(),
        "property node belongs to another Fdm"
    );
    assert_eq!(Error::ForeignNode.location(), None);
}

#[test]
fn an_fdms_own_nodes_read_and_write_by_handle() {
    let mut fdm = falling(1000.0);
    let altitude = node(&fdm, ALTITUDE);
    assert_eq!(fdm.node(ALTITUDE).unwrap(), Some(altitude));
    assert_eq!(
        fdm.read_node(altitude).unwrap(),
        fdm.get_property(ALTITUDE).unwrap()
    );
    let command = node(&fdm, COMMAND);
    assert!(fdm.write_node(command, 0.25).unwrap());
    assert_eq!(fdm.get_property(COMMAND).unwrap(), 0.25);
}

/// The fingerprint and the snapshot bytes know nothing of which executive
/// made them, so a snapshot restores onto another of the same model, whose
/// handles work on as before while the first executive's stay foreign.
#[test]
fn a_snapshot_restores_onto_another_fdm_with_its_own_handles() {
    let (mut a, mut b) = (falling(1000.0), falling(2000.0));
    assert_eq!(a.fingerprint().unwrap(), b.fingerprint().unwrap());
    let before_restore = node(&b, ALTITUDE);

    let mut snapshot = Snapshot::new();
    a.snapshot_into(&mut snapshot).unwrap();
    b.restore(&snapshot).unwrap();
    assert_eq!(
        b.read_node(before_restore).unwrap(),
        a.get_property(ALTITUDE).unwrap()
    );

    fly(&mut a, 25);
    fly(&mut b, 25);
    let altitude = a.get_property(ALTITUDE).unwrap();
    assert_eq!(node(&b, ALTITUDE), before_restore);
    assert_eq!(b.read_node(before_restore).unwrap(), altitude);

    let command = node(&b, COMMAND);
    assert!(b.write_node(command, 0.75).unwrap());
    assert_eq!(b.read_node(command).unwrap(), 0.75);
    fly(&mut b, 1);

    let theirs = node(&a, ALTITUDE);
    assert_eq!(b.read_node(theirs), Err(Error::ForeignNode));
    assert_eq!(b.write_node(theirs, 5.0), Err(Error::ForeignNode));
}
