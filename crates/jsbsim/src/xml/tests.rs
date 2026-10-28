// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use crate::{Error, MemoryResolver};

fn parse(text: &str) -> Element {
    parse_document(text.as_bytes(), "t.xml").unwrap()
}

#[test]
fn data_lines_are_trimmed_non_blank_lines() {
    let e = parse("<a>\n  1 2 \n\n\t 3\n</a>");
    assert_eq!(e.data_lines(), ["1 2", "3"]);
}

#[test]
fn text_after_a_child_belongs_to_the_parent_and_comments_do_not_split() {
    let e = parse("<a>x<!-- c -->y<b>in</b>z</a>");
    assert_eq!(e.data_lines(), ["xy", "z"]);
    assert_eq!(e.children()[0].data_lines(), ["in"]);
}

#[test]
fn entities_cdata_and_attribute_normalisation() {
    let e = parse(
        "<!DOCTYPE a [<!ENTITY on \"1\">]><a k=\"x\ty\n z\" v=\"&on;\">&on;&amp;<![CDATA[<raw>]]></a>",
    );
    assert_eq!(e.attribute("k"), "x y  z");
    assert_eq!(e.attribute("v"), "1");
    assert_eq!(e.data_lines(), ["1&<raw>"]);
}

#[test]
fn prefixed_attributes_and_namespace_declarations_keep_raw_names() {
    let e = parse(
        "<a xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"s\"><b/></a>",
    );
    assert_eq!(e.attribute("xsi:noNamespaceSchemaLocation"), "s");
    assert_eq!(
        e.attribute("xmlns:xsi"),
        "http://www.w3.org/2001/XMLSchema-instance"
    );
    assert!(e.children()[0].attributes().is_empty());
}

#[test]
fn latin1_documents_are_decoded() {
    let bytes = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><a>caf\xe9</a>";
    assert_eq!(
        parse_document(bytes, "t").unwrap().data_lines(),
        ["caf\u{e9}"]
    );
    let bad = b"<?xml version=\"1.0\"?><a>caf\xe9</a>";
    assert!(matches!(
        parse_document(bad, "t"),
        Err(XmlError::Encoding { .. })
    ));
}

#[test]
fn malformed_documents_report_position() {
    let err = parse_document(b"<a>\n<b></a>", "bad.xml").unwrap_err();
    assert!(
        matches!(err, XmlError::Malformed { line: 2, .. }),
        "{err:?}"
    );
}

#[test]
fn line_numbers_are_start_tag_lines() {
    let e = parse("<a>\n\n  <b\n x=\"1\"/>\n</a>");
    assert_eq!(e.line_number(), 1);
    assert_eq!(e.children()[0].line_number(), 3);
}

#[test]
fn strict_numbers() {
    assert_eq!(atof_locale_c(" -1.5e3 "), Ok(-1500.0));
    assert_eq!(atof_locale_c(".5"), Ok(0.5));
    assert_eq!(atof_locale_c("5."), Ok(5.0));
    for bad in ["1,5", "0x10", "inf", "1e", ".", "1 2", "+-1"] {
        assert!(atof_locale_c(bad).is_err(), "{bad}");
    }
    assert_eq!(
        atof_locale_c("  "),
        Err("Expecting a numeric attribute value, but only got spaces".to_owned())
    );
    assert_eq!(
        atof_locale_c("1e999"),
        Err("This number is too large: 1e999".to_owned())
    );
}

#[test]
fn data_as_number_needs_exactly_one_line() {
    let e = parse("<r><a>1\n2</a><b/><c>x</c></r>");
    assert!(matches!(
        e.find_element_value_as_number("a"),
        Err(XmlError::MultipleLines { lines: 2, .. })
    ));
    assert!(matches!(
        e.find_element_value_as_number("b"),
        Err(XmlError::NoData { .. })
    ));
    assert!(matches!(
        e.find_element_value_as_number("c"),
        Err(XmlError::InvalidNumber { .. })
    ));
    assert!(matches!(
        e.find_element_value_as_number("d"),
        Err(XmlError::MissingElement { .. })
    ));
}

#[test]
fn unit_conversion() {
    let e = parse(
        "<r><wingarea unit=\"M2\">10</wingarea><span>3</span><x unit=\"FURLONG\">1</x><y unit=\"LBS\">1</y></r>",
    );
    let m2_ft2 = crate::units::UnitTable::new().factor("M2", "FT2").unwrap();
    assert_eq!(
        e.find_element_value_as_number_convert_to("wingarea", "FT2"),
        Ok(10.0 * m2_ft2)
    );
    assert_eq!(
        e.find_element_value_as_number_convert_to("span", "FT"),
        Ok(3.0)
    );
    assert!(matches!(
        e.find_element_value_as_number_convert_to("x", "FT"),
        Err(XmlError::UnknownUnit { .. })
    ));
    assert!(matches!(
        e.find_element_value_as_number_convert_to("y", "FT"),
        Err(XmlError::Inconvertible { .. })
    ));
}

#[test]
fn triplets_accept_axis_aliases_and_default_to_zero() {
    let e = parse("<loc unit=\"IN\"><x>12</x><pitch>24</pitch></loc>");
    assert_eq!(e.find_element_triplet_convert_to("FT"), Ok([1.0, 2.0, 0.0]));
}

#[test]
fn find_elements_iterates_like_find_next_element() {
    let e = parse("<r><a>1</a><b/><a>2</a></r>");
    let values: Vec<&str> = e.find_elements("a").map(|a| a.data_line(0)).collect();
    assert_eq!(values, ["1", "2"]);
    assert_eq!(e.find_element("").map(Element::name), Some("a"));
    assert_eq!(e.num_elements(""), 3);
}

fn resolver() -> MemoryResolver {
    let mut r = MemoryResolver::new();
    r.insert("aircraft/x/x.xml", "<fdm_config/>");
    r.insert("aircraft/x/Systems/fcs.xml", "<system name=\"fcs\"/>");
    r.insert("systems/shared.xml", "<system name=\"shared\"/>");
    r.insert("engine/eng.xml", "<piston_engine/>");
    r.insert("aircraft/x/aero.xml", "<aerodynamics a=\"1\"/>");
    r
}

#[test]
fn includes_search_jsbsim_directories_in_order() {
    let r = resolver();
    let mut loader = DocumentLoader::new(&r, "aircraft/x");
    let mut system = parse("<system file=\"fcs\"/>");
    let doc = loader
        .open(&mut system, IncludeKind::System)
        .unwrap()
        .unwrap();
    assert_eq!(doc.attribute("name"), "fcs");
    assert!(
        system.children().is_empty(),
        "same root name is not appended"
    );
    let mut shared = parse("<system file=\"shared.xml\"/>");
    let doc = loader
        .open(&mut shared, IncludeKind::System)
        .unwrap()
        .unwrap();
    assert_eq!(doc.attribute("name"), "shared");
    let mut engine = parse("<engine file=\"eng\"/>");
    loader.open(&mut engine, IncludeKind::Engine).unwrap();
    assert_eq!(engine.children()[0].name(), "piston_engine");
    let mut plain = parse("<aerodynamics/>");
    assert_eq!(loader.open(&mut plain, IncludeKind::Model).unwrap(), None);
}

#[test]
fn missing_include_names_the_element_location() {
    let r = resolver();
    let mut loader = DocumentLoader::new(&r, "aircraft/x");
    let mut aero = parse("<aerodynamics file=\"nope\"/>");
    let err = loader.open(&mut aero, IncludeKind::Model).unwrap_err();
    assert_eq!(
        err,
        Error::Xml(XmlError::IncludeNotFound {
            file: "nope".into(),
            at: Location {
                file: "t.xml".into(),
                line: 1
            }
        })
    );
}

#[test]
fn load_document_adds_xml_extension_and_normalises() {
    let r = resolver();
    let loader = DocumentLoader::new(&r, "aircraft/x");
    assert_eq!(
        loader.load_document("aircraft/x/./x").unwrap().name(),
        "fdm_config"
    );
    assert_eq!(
        loader
            .load_document("aircraft/y/../x/x.xml")
            .unwrap()
            .name(),
        "fdm_config"
    );
}
