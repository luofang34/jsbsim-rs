// SPDX-License-Identifier: LGPL-2.1-or-later
//! A [`NodeId`] names the node of one path at one slot: it works on any
//! `Fdm` that holds that node there, and is refused, touching nothing, by
//! one that does not, such as an executive whose runtime nodes differ or one
//! a snapshot restore took the node from.

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

/// The `StaleNode` error for `node`, refused by both reads and writes, and
/// the path the refusing executive holds at its slot.
#[cfg(test)]
fn refused(fdm: &mut Fdm, node: NodeId) -> (u32, Option<String>) {
    let read = fdm.read_node(node).unwrap_err();
    assert_eq!(fdm.write_node(node, 5.0).unwrap_err(), read);
    let Error::StaleNode { slot, found } = read else {
        panic!("not StaleNode: {read:?}");
    };
    (slot, found)
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

/// Executives of one model load the same nodes into the same slots, so a
/// handle of one names the same property in the other.
#[test]
fn a_handle_names_the_same_property_in_another_fdm_of_the_model() {
    let (a, mut b) = (falling(1000.0), falling(2000.0));
    let theirs = node(&a, ALTITUDE);
    assert_eq!(theirs, node(&b, ALTITUDE));
    assert_eq!(
        b.read_node(theirs).unwrap(),
        b.get_property(ALTITUDE).unwrap()
    );
    assert!(b.write_node(node(&a, COMMAND), 0.5).unwrap());
    assert_eq!(b.get_property(COMMAND).unwrap(), 0.5);
}

#[test]
fn a_handle_to_a_node_another_fdm_holds_elsewhere_is_refused() {
    let (mut a, mut b) = (falling(1000.0), falling(1000.0));
    a.set_property("test/a", 1.0).unwrap();
    b.set_property("test/b", 2.0).unwrap();
    let theirs = node(&a, "test/a");
    let (slot, found) = refused(&mut b, theirs);
    assert_eq!(found.as_deref(), Some("/fdm/jsbsim/test/b"));
    assert_eq!(b.get_property("test/b").unwrap(), 2.0);
    assert_eq!(
        Error::StaleNode {
            slot,
            found: found.clone()
        }
        .to_string(),
        format!(
            "no property node at slot {slot} matches the handle; the slot holds /fdm/jsbsim/test/b"
        )
    );
    assert_eq!(Error::StaleNode { slot, found }.location(), None);
}

/// A restore drops the runtime nodes made after the snapshot; their handles
/// are refused, and stay refused when another node later takes the slot.
#[test]
fn a_handle_to_a_node_a_restore_dropped_is_refused() {
    let mut fdm = falling(1000.0);
    let mut snapshot = Snapshot::new();
    fdm.snapshot_into(&mut snapshot).unwrap();
    fdm.set_property("test/made-after", 3.0).unwrap();
    let dropped = node(&fdm, "test/made-after");
    fdm.restore(&snapshot).unwrap();
    let (slot, found) = refused(&mut fdm, dropped);
    assert_eq!(found, None);
    assert_eq!(
        Error::StaleNode { slot, found }.to_string(),
        format!("no property node at slot {slot} matches the handle; the slot is empty")
    );

    fdm.set_property("test/made-later", 4.0).unwrap();
    let (_, found) = refused(&mut fdm, dropped);
    assert_eq!(found.as_deref(), Some("/fdm/jsbsim/test/made-later"));
    assert_eq!(fdm.get_property("test/made-later").unwrap(), 4.0);
}

/// The fingerprint and the snapshot bytes know nothing of which executive
/// made them, so a snapshot restores onto another of the same model, whose
/// handles work on as before.
#[test]
fn a_snapshot_restores_onto_another_fdm_with_its_handles() {
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
    assert_eq!(node(&b, ALTITUDE), before_restore);
    assert_eq!(
        b.read_node(before_restore).unwrap(),
        a.get_property(ALTITUDE).unwrap()
    );
    let command = node(&b, COMMAND);
    assert!(b.write_node(command, 0.75).unwrap());
    assert_eq!(b.read_node(command).unwrap(), 0.75);
}
