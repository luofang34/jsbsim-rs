// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use crate::Error;
use crate::properties::Attribute;
use crate::xml::parse_document;

fn xml(text: &str) -> crate::xml::Element {
    parse_document(text.as_bytes(), "t.xml").unwrap()
}

struct Fixture {
    set: FunctionSet,
    tree: PropertyTree,
}

impl Fixture {
    fn new(props: &[(&str, f64)]) -> Self {
        let mut tree = PropertyTree::new();
        for (path, value) in props {
            let id = tree.get_or_create(tree.root(), path).unwrap();
            tree.set_double(id, *value);
        }
        Self {
            set: FunctionSet::new(),
            tree,
        }
    }

    fn load(&mut self, text: &str) -> Result<FnId, Error> {
        self.set.load_function(&mut self.tree, &xml(text), "")
    }

    fn eval(&mut self, id: FnId) -> Result<f64, Error> {
        Evaluator::new(&self.set, &mut self.tree).function(id)
    }

    fn value(&mut self, text: &str) -> f64 {
        let id = self.load(text).unwrap();
        self.eval(id).unwrap()
    }

    fn set(&mut self, path: &str, v: f64) {
        let id = self.tree.get(path).unwrap().unwrap();
        self.tree.set_double(id, v);
    }

    fn read(&mut self, path: &str) -> f64 {
        let id = self.tree.get(path).unwrap().unwrap();
        Evaluator::new(&self.set, &mut self.tree)
            .read_node(id)
            .unwrap()
    }
}

#[test]
fn operation_names_match_jsbsim() {
    let cpp = "product sum avg difference min max and or quotient pow toradians todegrees sqrt log2 ln log10 sign exp abs sin cos tan asin acos atan floor ceil fmod roundmultiple atan2 mod fraction integer lt le gt ge eq nq not ifthen random urandom switch interpolate1d rotation_alpha_local rotation_beta_local rotation_gamma_local rotation_bf_to_wf rotation_wf_to_bf";
    assert_eq!(OPERATIONS.join(" "), cpp);
}

#[test]
fn arithmetic_and_sentinels() {
    let mut f = Fixture::new(&[("x", 2.0)]);
    assert_eq!(
        f.value("<function><sum><v>1</v><p>x</p><v>-0.5</v></sum></function>"),
        2.5
    );
    assert_eq!(
        f.value("<function><difference><v>1</v><p>x</p><p>-x</p></difference></function>"),
        1.0
    );
    assert_eq!(
        f.value("<function><quotient><p>x</p><v>0</v></quotient></function>"),
        f64::INFINITY
    );
    assert_eq!(
        f.value("<function><quotient><p>-x</p><v>0</v></quotient></function>"),
        f64::INFINITY
    );
    assert_eq!(
        f.value("<function><sqrt><p>-x</p></sqrt></function>"),
        f64::NEG_INFINITY
    );
    assert_eq!(
        f.value("<function><ln><v>0</v></ln></function>"),
        f64::NEG_INFINITY
    );
    assert_eq!(
        f.value("<function><mod><v>7.9</v><p>x</p></mod></function>"),
        1.0
    );
    assert_eq!(
        f.value("<function><integer><v>-2.5</v></integer></function>"),
        -2.0
    );
    assert_eq!(
        f.value("<function><fraction><v>-2.5</v></fraction></function>"),
        -0.5
    );
    assert_eq!(
        f.value("<function><roundmultiple><v>7.4</v><v>5</v></roundmultiple></function>"),
        5.0
    );
    assert_eq!(
        f.value("<function><max><v>1</v><p>x</p></max></function>"),
        2.0
    );
    assert_eq!(
        f.value("<function><sign><v>-0.0</v></sign></function>"),
        1.0
    );
}

#[test]
fn logical_operands_must_be_binary() {
    let mut f = Fixture::new(&[("half", 0.5), ("two", 2.0)]);
    assert_eq!(
        f.value("<function><not><p>half</p></not></function>"),
        0.0,
        "0.5 counts as true"
    );
    let id = f
        .load("<function><not><p>two</p></not></function>")
        .unwrap();
    assert!(matches!(
        f.eval(id),
        Err(Error::Function(FunctionError::NotBinary { .. }))
    ));
    assert_eq!(
        f.value("<function><or><v>1</v><p>two</p></or></function>"),
        1.0,
        "short-circuit"
    );
}

#[test]
fn interpolate1d_and_switch() {
    let mut f = Fixture::new(&[("x", 0.5)]);
    let id = f
        .load("<function><interpolate1d><p>x</p><v>0</v><v>-1</v><v>1</v><v>0</v><v>3</v><v>0</v><v>4</v><v>2</v></interpolate1d></function>")
        .unwrap();
    assert_eq!(f.eval(id).unwrap(), -0.5);
    f.set("x", 3.5);
    assert_eq!(f.eval(id).unwrap(), 1.0);
    let s = f
        .load("<function><switch><p>x</p><v>10</v><v>20</v></switch></function>")
        .unwrap();
    f.set("x", 0.6);
    assert_eq!(f.eval(s).unwrap(), 20.0);
    f.set("x", 1.6);
    assert!(f.eval(s).is_err());
}

#[test]
fn named_functions_are_read_only_properties_evaluated_on_read() {
    let mut f = Fixture::new(&[("x", 1.0)]);
    f.load("<function name=\"out\"><product><p>x</p><v>3</v></product></function>")
        .unwrap();
    assert_eq!(f.read("out"), 3.0);
    f.set("x", 2.0);
    assert_eq!(f.read("out"), 6.0);
    let node = f.tree.get("out").unwrap().unwrap();
    assert!(f.tree.is_tied(node));
    assert!(!f.tree.set_double(node, 0.0));
    assert!(matches!(
        f.load("<function name=\"out\"><v>1</v></function>"),
        Err(Error::Function(FunctionError::AlreadyBound { .. }))
    ));
}

#[test]
fn constant_operations_fold_and_cascade() {
    let mut f = Fixture::new(&[]);
    f.load("<function><sum name=\"c1\"><v>1</v><v>2</v></sum></function>")
        .unwrap();
    let c1 = f.tree.get("c1").unwrap().unwrap();
    assert!(!f.tree.is_tied(c1));
    assert!(!f.tree.attribute(c1, Attribute::WRITE));
    assert_eq!(f.tree.get_double(c1), 3.0);
    // c1 is now an untied read-only constant, so this folds too.
    f.load("<function><product name=\"c2\"><p>c1</p><v>2</v></product></function>")
        .unwrap();
    let c2 = f.tree.get("c2").unwrap().unwrap();
    assert!(!f.tree.is_tied(c2));
    assert_eq!(f.tree.get_double(c2), 6.0);
}

#[test]
fn single_argument_variadics_collapse() {
    let mut f = Fixture::new(&[("x", 4.0)]);
    assert_eq!(
        f.value("<function><product><sum><p>x</p></sum></product></function>"),
        4.0
    );
    assert!(matches!(
        f.load("<function><and><p>x</p></and></function>"),
        Err(Error::Function(FunctionError::Arguments { .. }))
    ));
}

#[test]
fn argument_checks() {
    let mut f = Fixture::new(&[("x", 4.0)]);
    for text in [
        "<function><pow><v>1</v></pow></function>",
        "<function><sin><v>1</v><v>2</v></sin></function>",
        "<function><interpolate1d><p>x</p><v>0</v><v>1</v><v>2</v><v>3</v><v>4</v></interpolate1d></function>",
        "<function><v>1</v><v>2</v></function>",
    ] {
        assert!(
            matches!(
                f.load(text),
                Err(Error::Function(FunctionError::Arguments { .. }))
            ),
            "{text}"
        );
    }
}

#[test]
fn random_is_unsupported_and_unknown_operations_are_ignored() {
    let mut f = Fixture::new(&[]);
    assert!(matches!(
        f.load("<function><random/></function>"),
        Err(Error::Unsupported { construct, .. }) if construct == "<random>"
    ));
    assert_eq!(
        f.value("<function><sum><bogus/><v>1</v><v>2</v></sum></function>"),
        3.0
    );
}

#[test]
fn late_bound_properties_error_only_when_still_missing() {
    let mut f = Fixture::new(&[]);
    let id = f.load("<function><p>later</p></function>").unwrap();
    assert!(matches!(
        f.eval(id),
        Err(Error::Function(FunctionError::MissingProperty { .. }))
    ));
    let node = f.tree.get_or_create(f.tree.root(), "later").unwrap();
    f.tree.set_double(node, 7.0);
    assert_eq!(f.eval(id).unwrap(), 7.0);
}

#[test]
fn templates_apply_to_a_property() {
    let mut f = Fixture::new(&[("aero/alpha-rad", 0.5)]);
    let template = xml("<function name=\"deg\"><product><p>#</p><v>2</v></product></function>");
    f.set.add_template(&mut f.tree, "deg", &template).unwrap();
    assert_eq!(
        f.value("<function><p apply=\"deg\">aero/alpha-rad</p></function>"),
        1.0
    );
}

#[test]
fn tables_interpolate_and_clamp() {
    let mut f = Fixture::new(&[("r", 0.5), ("c", 15.0), ("t", 1.0)]);
    let one = f
        .load("<function><table><independentVar>r</independentVar><tableData>0 0\n1 10\n2 -10</tableData></table></function>")
        .unwrap();
    assert_eq!(f.eval(one).unwrap(), 5.0);
    f.set("r", 9.0);
    assert_eq!(f.eval(one).unwrap(), -10.0);
    let two = f
        .load("<function><table><independentVar lookup=\"row\">r</independentVar><independentVar lookup=\"column\">c</independentVar><tableData>10 20\n0 1 2\n10 3 4</tableData></table></function>")
        .unwrap();
    f.set("r", 5.0);
    assert_eq!(f.eval(two).unwrap(), 2.5);
    let three = f
        .load("<function><table><independentVar lookup=\"row\">r</independentVar><independentVar lookup=\"column\">c</independentVar><independentVar lookup=\"table\">t</independentVar><tableData breakPoint=\"0\">10 20\n0 1 2\n10 3 4</tableData><tableData breakPoint=\"2\">10 20\n0 11 12\n10 13 14</tableData></table></function>")
        .unwrap();
    assert_eq!(f.eval(three).unwrap(), 7.5);
}

#[test]
fn table_errors() {
    let mut f = Fixture::new(&[("r", 0.0)]);
    for text in [
        "<function><table type=\"qsdfghjkl\"><independentVar>r</independentVar><tableData>0 0</tableData></table></function>",
        "<function><table><tableData>0 0</tableData></table></function>",
        "<function><table><independentVar>r</independentVar><tableData>0 0\n0 1</tableData></table></function>",
        "<function><table><independentVar>r</independentVar><tableData>0 0\n1</tableData></table></function>",
        "<function><table><independentVar lookup=\"sideways\">r</independentVar><tableData>0 0</tableData></table></function>",
    ] {
        assert!(
            matches!(
                f.load(text),
                Err(Error::Function(FunctionError::Table { .. }))
            ),
            "{text}"
        );
    }
    assert!(matches!(
        f.load("<function><table type=\"internal\"><tableData>0 0</tableData></table></function>"),
        Err(Error::Function(
            FunctionError::InternalTableInFunction { .. }
        ))
    ));
}

#[test]
fn conditions() {
    let mut f = Fixture::new(&[("a", 1.0), ("b", 2.0)]);
    let el = xml("<test logic=\"OR\">a GT 5\nb == 2<test>a lt b</test></test>");
    let c = Condition::load(&el, &f.tree).unwrap();
    assert!(
        c.evaluate(&mut Evaluator::new(&f.set, &mut f.tree))
            .unwrap()
    );
    let tab = xml("<test>a\tGT 5</test>");
    assert!(
        Condition::load(&tab, &f.tree).is_err(),
        "tabs do not separate"
    );
    let late = xml("<test logic=\"OR\">a == 1\nmissing == 1</test>");
    let c = Condition::load(&late, &f.tree).unwrap();
    assert!(
        c.evaluate(&mut Evaluator::new(&f.set, &mut f.tree))
            .is_err(),
        "no short-circuit"
    );
}
