// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

fn tree_with(path: &str) -> (PropertyTree, NodeId) {
    let mut tree = PropertyTree::new();
    let id = tree.get_or_create(tree.root(), path).unwrap();
    (tree, id)
}

#[test]
fn creates_and_finds_nested_indexed_nodes() {
    let (tree, id) = tree_with("/fcs/engine[2]/throttle");
    assert_eq!(tree.get("fcs/engine[2]/throttle").unwrap(), Some(id));
    assert_eq!(tree.get("fcs/engine/throttle").unwrap(), None);
    assert_eq!(tree.fully_qualified_name(id), "/fcs/engine[2]/throttle");
    assert_eq!(tree.path(id, false), "/fcs[0]/engine[2]/throttle[0]");
    assert_eq!(tree.fully_qualified_name(tree.root()), "/");
    assert_eq!(tree.path(tree.root(), false), "");
}

#[test]
fn relative_paths_and_dots() {
    let (mut tree, id) = tree_with("a/b/c");
    let b = tree.parent(id).unwrap();
    assert_eq!(tree.find(b, "c").unwrap(), Some(id));
    assert_eq!(tree.find(id, "../c").unwrap(), Some(id));
    assert_eq!(tree.find(id, "./.").unwrap(), Some(id));
    assert_eq!(tree.find(id, "/a//b///c/").unwrap(), Some(id));
    assert_eq!(tree.find(id, "").unwrap(), Some(id));
    let d = tree.get_or_create(id, "../d").unwrap();
    assert_eq!(tree.fully_qualified_name(d), "/a/b/d");
}

#[test]
fn index_after_name_starts_a_new_component() {
    let (tree, id) = tree_with("a[1]b");
    assert_eq!(tree.fully_qualified_name(id), "/a[1]/b");
}

#[test]
fn explicit_index_overrides_last_component() {
    let mut tree = PropertyTree::new();
    let id = tree
        .get_or_create_indexed(tree.root(), "x/y[3]", Some(5))
        .unwrap();
    assert_eq!(tree.fully_qualified_name(id), "/x/y[5]");
}

#[test]
fn rejects_malformed_paths_with_simgear_messages() {
    let tree = PropertyTree::new();
    let message = |path: &str| tree.get(path).unwrap_err().message;
    assert_eq!(
        message("1abc"),
        "name must begin with alpha or '_' in '1abc'"
    );
    assert_eq!(
        message("a b"),
        "name may contain only ._- and alphanumeric characters in 'a b'"
    );
    assert_eq!(
        message("a[2"),
        "unterminated index (looking for ']') in 'a[2'"
    );
    assert_eq!(message(".x"), "Illegal character after . in '.x'");
    assert_eq!(message("..."), "Illegal character after .. in '...'");
    assert_eq!(message(".."), "Attempt to move past root with '..'");
}

#[test]
fn has_node_ignores_leading_minus() {
    let (tree, _) = tree_with("velocities/u-fps");
    assert!(tree.has_node("-velocities/u-fps").unwrap());
    assert!(!tree.has_node("velocities/v-fps").unwrap());
}

#[test]
fn first_typed_write_fixes_the_type() {
    let (mut tree, id) = tree_with("x");
    assert_eq!(tree.value_type(id), ValueType::None);
    assert!(tree.set_int(id, 3));
    assert_eq!(tree.value_type(id), ValueType::Int);
    assert!(tree.set_double(id, 2.9));
    assert_eq!(tree.get_double(id), 2.0);
    assert!(tree.set_string(id, "17abc"));
    assert_eq!(tree.get_int(id), 17);
}

#[test]
fn double_reads_of_other_types() {
    let (mut tree, id) = tree_with("s");
    tree.set_string(id, "  2.5e1xyz");
    assert_eq!(tree.get_double(id), 25.0);
    let empty = tree.get_or_create(tree.root(), "t").unwrap();
    assert!(!tree.get_bool(empty));
    let b = tree.get_or_create(tree.root(), "b").unwrap();
    tree.set_bool(b, true);
    assert_eq!(tree.get_double(b), 1.0);
    let t = tree.get_or_create(tree.root(), "word").unwrap();
    tree.set_string(t, "true");
    assert!(tree.get_bool(t));
    assert_eq!(tree.get_double(t), 0.0);
}

#[test]
fn strings_of_numbers_use_simgear_formats() {
    let (mut tree, id) = tree_with("d");
    tree.set_double(id, 1.0 / 3.0);
    assert_eq!(tree.get_string(id), "0.3333333333");
    let s = tree.get_or_create(tree.root(), "s").unwrap();
    tree.set_string(s, "");
    tree.set_double(s, 0.5);
    assert_eq!(tree.get_string(s), "0.500000");
}

#[test]
fn unspecified_takes_the_type_of_the_first_typed_write() {
    let (mut tree, id) = tree_with("u");
    tree.set_unspecified(id, "4");
    assert_eq!(tree.value_type(id), ValueType::Unspecified);
    tree.set_bool(id, true);
    assert_eq!(tree.value_type(id), ValueType::Bool);
}

#[test]
fn read_and_write_attributes_gate_access() {
    let (mut tree, id) = tree_with("x");
    tree.set_double(id, 5.0);
    tree.set_attribute(id, Attribute::WRITE, false);
    assert!(!tree.set_double(id, 6.0));
    assert_eq!(tree.get_double(id), 5.0);
    tree.set_attribute(id, Attribute::READ, false);
    assert_eq!(tree.get_double(id), 0.0);
    assert_eq!(tree.get_string(id), "");
}

#[test]
fn aliases_forward_reads_and_writes() {
    let mut tree = PropertyTree::new();
    let target = tree.get_or_create(tree.root(), "real").unwrap();
    let alias = tree.get_or_create(tree.root(), "alias").unwrap();
    tree.set_double(target, 1.5);
    assert!(tree.alias(alias, target));
    assert_eq!(tree.get_double(alias), 1.5);
    assert!(tree.set_double(alias, 2.5));
    assert_eq!(tree.get_double(target), 2.5);
    assert_eq!(tree.value_type(alias), ValueType::Double);
    assert!(tree.alias(alias, target), "same alias again is accepted");
    let other = tree.get_or_create(tree.root(), "other").unwrap();
    assert!(!tree.alias(alias, other), "already aliasing another node");
    assert!(tree.unalias(alias));
    assert_eq!(tree.value_type(alias), ValueType::None);
}

#[test]
fn alias_cycles_are_refused() {
    let mut tree = PropertyTree::new();
    let a = tree.get_or_create(tree.root(), "a").unwrap();
    let b = tree.get_or_create(tree.root(), "b").unwrap();
    assert!(!tree.alias(a, a));
    assert!(tree.alias(a, b));
    assert!(!tree.alias(b, a));
}

#[test]
fn read_only_ties_reject_outside_writes_but_not_the_owner() {
    let (mut tree, id) = tree_with("velocities/vc-kts");
    assert!(tree.tie(id, Value::Double(42.0), false));
    assert!(!tree.set_double(id, 1.0));
    tree.store(id, 7.0);
    assert_eq!(tree.get_double(id), 7.0);
    assert!(!tree.tie(id, Value::Double(0.0), true), "already tied");
    assert!(tree.untie(id));
    assert!(tree.attribute(id, Attribute::WRITE));
    assert_eq!(tree.get_double(id), 7.0);
}

#[test]
fn naming_helpers() {
    assert_eq!(mk_property_name("Left Main Gear", true), "left-main-gear");
    assert_eq!(mk_property_name("Left Main", false), "Left-Main");
    assert_eq!(printable_name("left_main_gear"), "left main gear");
    let (tree, id) = tree_with("fcs/elevator-cmd-norm");
    assert_eq!(tree.relative_name(id, "/fcs/"), "elevator-cmd-norm");
    assert_eq!(tree.relative_name(id, "/aero/"), "/fcs/elevator-cmd-norm");
}

#[test]
fn the_lookup_generation_moves_only_when_a_lookup_could_change() {
    let (mut tree, id) = tree_with("a/b");
    let created = tree.generation();
    tree.get_or_create(tree.root(), "a/b").unwrap();
    tree.set_double(id, 2.0);
    tree.set_attribute(id, Attribute::PRESERVE, true);
    assert_eq!(tree.get("a/c").unwrap(), None);
    assert_eq!(tree.generation(), created);
    let c = tree.get_or_create(tree.root(), "a/c").unwrap();
    assert_ne!(tree.generation(), created);
    let after_create = tree.generation();
    tree.set_attribute(c, Attribute::REMOVED, true);
    assert_eq!(tree.get("a/c").unwrap(), None);
    assert_ne!(tree.generation(), after_create);
}

#[test]
fn node_maps_index_by_node_and_iterate_in_node_order() {
    let mut tree = PropertyTree::new();
    let ids: Vec<NodeId> = ["x", "y", "z"]
        .iter()
        .map(|p| tree.get_or_create(tree.root(), p).unwrap())
        .collect();
    let mut map = NodeMap::new();
    assert!(map.is_empty());
    map.insert(ids[2], 3.0);
    map.insert(ids[0], 1.0);
    assert_eq!(map.insert(ids[0], 10.0), Some(1.0));
    assert!(map.contains(ids[2]) && !map.contains(ids[1]));
    assert_eq!(map.get(ids[1]), None);
    let entries: Vec<_> = map.iter().map(|(n, v)| (n, *v)).collect();
    assert_eq!(entries, [(ids[0], 10.0), (ids[2], 3.0)]);
    assert_eq!(map.remove(ids[2]), Some(3.0));
    assert_eq!(map.remove(ids[1]), None);
    assert_eq!(map.iter().count(), 1);
}

#[test]
fn ids_from_another_tree_read_as_an_unreadable_detached_node() {
    let (big, id) = tree_with("a/b/c");
    let mut small = PropertyTree::new();
    assert!(big.nodes.len() > small.nodes.len());
    assert_eq!(small.get_double(id), 0.0);
    assert!(!small.get_bool(id));
    assert!(!small.set_double(id, 1.0));
    assert!(!small.tie(id, Value::Double(1.0), true));
    let root = small.root();
    assert!(!small.alias(id, root));
    assert!(small.children(id).is_empty());
    assert_eq!(small.parent(id), None);
    assert!(small.find(id, "x").unwrap().is_none());
    assert!(!small.attribute(id, Attribute::READ));
    assert!(small.get_or_create(id, "x").is_err());
    let before = small.nodes.len();
    let child = small.get_or_create(root, "x").unwrap();
    assert_eq!(small.nodes.len(), before + 1);
    assert_eq!(small.parent(child), Some(root));
}

#[test]
fn absolute_paths_leave_an_instance_tree() {
    let mut tree = PropertyTree::for_instance(0);
    let base = tree.root();
    assert_ne!(base, tree.true_root());
    assert_eq!(tree.fully_qualified_name(base), "/fdm/jsbsim");
    let relative = tree.get_or_create(base, "systems/x").unwrap();
    let absolute = tree.get_or_create(base, "/systems/x").unwrap();
    assert_ne!(relative, absolute);
    assert_eq!(tree.fully_qualified_name(relative), "/fdm/jsbsim/systems/x");
    assert_eq!(tree.fully_qualified_name(absolute), "/systems/x");
    assert_eq!(tree.relative_name(relative, "/fdm/jsbsim/"), "systems/x");
    assert_eq!(
        tree.get("/fdm/jsbsim[0]/systems/x").unwrap(),
        Some(relative)
    );
    assert_eq!(tree.get("../../systems/x").unwrap(), Some(absolute));
    assert_eq!(tree.children(base), &[tree.parent(relative).unwrap()]);
}

#[test]
fn a_double_written_at_its_target_converts_to_the_node_type() {
    let mut tree = PropertyTree::new();
    let mut typed = |path: &str, set: fn(&mut PropertyTree, NodeId)| {
        let id = tree.get_or_create(tree.root(), path).unwrap();
        set(&mut tree, id);
        let target = tree.write_target(id).unwrap();
        tree.set_double_at_target(target, 2.75);
        (tree.value_type(id), tree.get_string(id))
    };
    let expected = [
        (ValueType::Bool, "true"),
        (ValueType::Int, "2"),
        (ValueType::Long, "2"),
        (ValueType::Float, "2.75"),
        (ValueType::Double, "2.75"),
        (ValueType::String, "2.750000"),
        (ValueType::Double, "2.75"),
    ];
    let got = [
        typed("b", |t, id| assert!(t.set_bool(id, false))),
        typed("i", |t, id| assert!(t.set_int(id, 0))),
        typed("l", |t, id| assert!(t.set_long(id, 0))),
        typed("f", |t, id| assert!(t.set_float(id, 0.0))),
        typed("d", |t, id| assert!(t.set_double(id, 0.0))),
        typed("s", |t, id| assert!(t.set_string(id, "x"))),
        typed("none", |_, _| {}),
    ];
    for ((ty, text), (got_ty, got_text)) in expected.into_iter().zip(got) {
        assert_eq!((ty, text), (got_ty, got_text.as_str()));
    }
}
