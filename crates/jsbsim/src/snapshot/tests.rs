// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use crate::fdm::Fdm;
use crate::math::Location;
use crate::properties::{NodeId, Value};

fn written(mut f: impl FnMut(&mut Writer<'_>) -> Result<(), SnapshotError>) -> Vec<u8> {
    let mut out = Vec::new();
    f(&mut Writer::new(&mut out)).unwrap();
    out
}

#[test]
fn values_are_little_endian_and_doubles_keep_their_bits() {
    let nan = f64::from_bits(0x7ff8_dead_beef_0001);
    let mut values = (nan, (-0.0f64, (7u32, (true, -2i32))));
    let bytes = written(|w| {
        values.0.visit(w)?;
        values.1.0.visit(w)?;
        values.1.1.0.visit(w)?;
        values.1.1.1.0.visit(w)?;
        values.1.1.1.1.visit(w)
    });
    let mut expected = Vec::new();
    expected.extend_from_slice(&0x7ff8_dead_beef_0001u64.to_le_bytes());
    expected.extend_from_slice(&0x8000_0000_0000_0000u64.to_le_bytes());
    expected.extend_from_slice(&7u32.to_le_bytes());
    expected.push(1);
    expected.extend_from_slice(&(-2i32 as u32).to_le_bytes());
    assert_eq!(bytes, expected);

    let mut back = (0.0f64, 1.0f64, 0u32, false, 0i32);
    let mut r = Reader::new(&bytes, 0);
    back.0.visit(&mut r).unwrap();
    back.1.visit(&mut r).unwrap();
    back.2.visit(&mut r).unwrap();
    back.3.visit(&mut r).unwrap();
    back.4.visit(&mut r).unwrap();
    r.finish().unwrap();
    assert_eq!(back.0.to_bits(), nan.to_bits());
    assert_eq!(back.1.to_bits(), (-0.0f64).to_bits());
    assert_eq!((back.2, back.3, back.4), (7, true, -2));
}

#[test]
fn optional_state_and_nodes_round_trip() {
    let mut some: Option<f64> = Some(2.5);
    let mut none: Option<f64> = None;
    let mut node: Option<NodeId> = Some(NodeId::from_u32(42));
    let mut no_node: Option<NodeId> = None;
    let bytes = written(|w| {
        some.visit(w)?;
        none.visit(w)?;
        node.visit(w)?;
        no_node.visit(w)
    });
    let (mut a, mut b, mut c, mut d) = (None, Some(9.0), None, Some(NodeId::from_u32(1)));
    let mut r = Reader::new(&bytes, 0);
    a.visit(&mut r).unwrap();
    b.visit(&mut r).unwrap();
    c.visit(&mut r).unwrap();
    d.visit(&mut r).unwrap();
    assert_eq!(
        (a, b, c, d),
        (Some(2.5), None, Some(NodeId::from_u32(42)), None)
    );
}

#[test]
fn bad_values_and_short_input_are_typed_errors() {
    let mut flag = false;
    assert_eq!(
        flag.visit(&mut Reader::new(&[2], 0)),
        Err(SnapshotError::InvalidValue {
            what: "bool",
            value: 2
        })
    );
    let mut x = 0.0f64;
    assert_eq!(
        x.visit(&mut Reader::new(&[0; 3], 0)),
        Err(SnapshotError::Truncated {
            offset: 0,
            needed: 8,
            len: 3
        })
    );
    let mut list = vec![1.0f64, 2.0];
    let bytes = written(|w| vec![1.0f64].visit(w));
    assert_eq!(
        list.visit(&mut Reader::new(&bytes, 0)),
        Err(SnapshotError::Shape {
            what: "list entries",
            expected: 2,
            found: 1
        })
    );
    assert_eq!(
        Reader::new(&[0; 4], 2).finish(),
        Err(SnapshotError::TrailingBytes { extra: 2 })
    );
}

#[test]
fn a_dynamic_list_takes_the_saved_length() {
    let mut saved = vec![1.0f64, 2.0, 3.0];
    let bytes = written(|w| visit_dynamic(&mut saved, w, "values"));
    let mut list = vec![9.0f64];
    visit_dynamic(&mut list, &mut Reader::new(&bytes, 0), "values").unwrap();
    assert_eq!(list, saved);
}

#[test]
fn the_header_is_checked_before_the_body() {
    let mut fdm = Fdm::new().unwrap();
    let mut snapshot = Snapshot::new();
    fdm.snapshot_into(&mut snapshot).unwrap();
    let bytes = snapshot.as_bytes().to_vec();
    assert_eq!(bytes.get(..8), Some(&MAGIC[..]));
    assert_eq!(bytes.get(8..12), Some(&FORMAT_VERSION.to_le_bytes()[..]));
    assert_eq!(Snapshot::from_bytes(&bytes).unwrap(), snapshot);

    let mut wrong_version = bytes.clone();
    wrong_version[8] ^= 0xff;
    assert_eq!(
        Snapshot::from_bytes(&wrong_version),
        Err(SnapshotError::VersionMismatch {
            found: FORMAT_VERSION ^ 0xff,
            expected: FORMAT_VERSION
        })
    );
    let mut wrong_magic = bytes.clone();
    wrong_magic[0] = b'X';
    assert!(matches!(
        Snapshot::from_bytes(&wrong_magic),
        Err(SnapshotError::BadMagic { .. })
    ));
    assert!(matches!(
        Snapshot::from_bytes(&bytes[..20]),
        Err(SnapshotError::Truncated { .. })
    ));
    let mut longer = bytes.clone();
    longer.push(0);
    let longer = Snapshot::from_bytes(&longer).unwrap();
    assert_eq!(
        fdm.restore(&longer),
        Err(crate::Error::Snapshot(SnapshotError::TrailingBytes {
            extra: 1
        }))
    );
}

#[test]
fn an_unloaded_executive_restores_its_own_snapshot() {
    let mut fdm = Fdm::new().unwrap();
    let mut snapshot = Snapshot::new();
    fdm.snapshot_into(&mut snapshot).unwrap();
    fdm.set_property("simulation/dt", 0.5).unwrap();
    fdm.set_property("test/created", 3.0).unwrap();
    fdm.restore(&snapshot).unwrap();
    assert_eq!(fdm.node("test/created").unwrap(), None);
    let mut again = Snapshot::new();
    fdm.snapshot_into(&mut again).unwrap();
    assert_eq!(again, snapshot);
}

#[test]
fn the_fingerprint_names_the_first_part_that_differs() {
    let base = Fingerprint {
        documents: [1; 32],
        tree: [2; 32],
        models: [3; 32],
        load_nodes: 10,
    };
    assert_eq!(base.first_difference(&base), None);
    let models = Fingerprint {
        models: [0; 32],
        ..base
    };
    assert_eq!(
        base.first_difference(&models),
        Some(FingerprintPart::Models)
    );
    let tree = Fingerprint {
        load_nodes: 11,
        ..models
    };
    assert_eq!(base.first_difference(&tree), Some(FingerprintPart::Tree));
    let documents = Fingerprint {
        documents: [0; 32],
        ..tree
    };
    assert_eq!(
        base.first_difference(&documents),
        Some(FingerprintPart::Documents)
    );
}

#[test]
fn nodes_created_before_a_snapshot_are_made_again() {
    let mut fdm = Fdm::new().unwrap();
    fdm.set_property("test/a", 1.0).unwrap();
    fdm.set_property("test/b[2]", 2.0).unwrap();
    let mut snapshot = Snapshot::new();
    fdm.snapshot_into(&mut snapshot).unwrap();
    let mut other = Fdm::new().unwrap();
    other.set_property("test/c", 5.0).unwrap();
    other.restore(&snapshot).unwrap();
    assert_eq!(other.node("test/c").unwrap(), None);
    assert_eq!(other.get_property("test/a").unwrap(), 1.0);
    assert_eq!(other.get_property("test/b[2]").unwrap(), 2.0);
    assert_eq!(other.node("test/a").unwrap(), fdm.node("test/a").unwrap());
}

/// A list whose length is state.
#[derive(Clone)]
struct Dynamic(Vec<f64>);

impl Visit for Dynamic {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        visit_dynamic(&mut self.0, v, "values")
    }
}

fn encoded<T: Visit + Clone>(x: &T) -> Vec<u8> {
    let mut x = x.clone();
    written(|w| x.visit(w))
}

/// Checks `bytes` onto `target`, which must consume them all and leave
/// it as it was, then reads them onto it, which must consume the same
/// bytes and leave what they encode.
fn checked_then_read<T: Visit + Clone>(target: &T, bytes: &[u8]) {
    let mut checked = target.clone();
    let mut c = Checker::new(Reader::new(bytes, 0));
    checked.visit(&mut c).unwrap();
    c.finish().unwrap();
    assert_eq!(encoded(&checked), encoded(target));
    let mut read = target.clone();
    let mut r = Reader::new(bytes, 0);
    read.visit(&mut r).unwrap();
    r.finish().unwrap();
    assert_eq!(encoded(&read), bytes);
}

#[test]
fn a_check_follows_the_stream_where_the_shapes_differ_and_writes_nothing() {
    checked_then_read(&None::<f64>, &encoded(&Some(2.5f64)));
    checked_then_read(&Some(1.0f64), &encoded(&None::<f64>));
    checked_then_read(&Dynamic(vec![9.0]), &encoded(&Dynamic(vec![1.0, 2.0, 3.0])));
    checked_then_read(&Dynamic(vec![1.0, 2.0, 3.0]), &encoded(&Dynamic(vec![9.0])));
    checked_then_read(
        &Value::Double(1.0),
        &encoded(&Value::String("text".to_owned())),
    );
    checked_then_read(&Value::None, &encoded(&Value::Bool(true)));
    let plain = Location::from_spherical(0.1, 0.2, 2.1e7);
    let mut ellipsoidal = Location::from_spherical(0.3, 0.4, 2.2e7);
    ellipsoidal.set_ellipse(2.09e7, 2.08e7);
    checked_then_read(&plain, &encoded(&ellipsoidal));
    checked_then_read(&ellipsoidal, &encoded(&plain));
}

#[test]
fn a_check_rejects_what_a_restore_would_and_keeps_the_value() {
    let check = |bytes| Checker::new(Reader::new(bytes, 0));
    let mut flag = true;
    assert_eq!(
        flag.visit(&mut check(&[2])),
        Err(SnapshotError::InvalidValue {
            what: "bool",
            value: 2
        })
    );
    flag.visit(&mut check(&[0])).unwrap();
    assert!(flag);
    let mut value = Value::Double(1.0);
    assert_eq!(
        value.visit(&mut check(&[9])),
        Err(SnapshotError::InvalidValue {
            what: "property value type",
            value: 9
        })
    );
    assert_eq!(value, Value::Double(1.0));
    let mut text = "kept".to_owned();
    assert_eq!(
        text.visit(&mut check(&[1, 0, 0, 0, 0xff])),
        Err(SnapshotError::InvalidText { offset: 4 })
    );
    text.visit(&mut check(&[3, 0, 0, 0, b'n', b'e', b'w']))
        .unwrap();
    assert_eq!(text, "kept");
}
