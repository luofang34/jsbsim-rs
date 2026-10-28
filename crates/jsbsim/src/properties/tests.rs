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
