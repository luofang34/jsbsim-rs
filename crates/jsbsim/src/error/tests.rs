// SPDX-License-Identifier: LGPL-2.1-or-later
use std::rc::Rc;

use super::{Error, FunctionError, SourceLocation, XmlError};
use crate::xml::Element;
use crate::{Fdm, MemoryResolver};

fn element() -> Element {
    Element::new("thing", Rc::from("aircraft/box.xml"), 12)
}

fn at() -> SourceLocation {
    SourceLocation {
        file: "aircraft/box.xml".to_owned(),
        line: 12,
    }
}

#[test]
fn a_model_error_at_an_element_says_where_once() {
    let e = element().model_error("bad thing");
    assert_eq!(
        e.to_string(),
        "In file aircraft/box.xml: line 12: bad thing"
    );
    assert_eq!(e.location(), Some(&at()));
    let Error::Model { message, .. } = &e else {
        panic!("{e:?}");
    };
    assert_eq!(message, "bad thing");
}

#[test]
fn a_model_error_without_an_element_is_the_bare_message() {
    let e = Error::model("no engine");
    assert_eq!(e.to_string(), "no engine");
    assert_eq!(e.location(), None);
}

#[test]
fn model_at_equals_the_element_constructor() {
    assert_eq!(
        Error::model_at(at(), "bad thing"),
        element().model_error("bad thing")
    );
}

#[test]
fn unsupported_carries_its_location() {
    let e = element().unsupported("<child>");
    assert_eq!(
        e.to_string(),
        "In file aircraft/box.xml: line 12: unsupported <child>"
    );
    assert_eq!(e.location(), Some(&at()));
    assert_eq!(
        e,
        Error::Unsupported {
            construct: "<child>".to_owned(),
            at: at()
        }
    );
}

#[test]
fn unknown_input_type_carries_its_location() {
    let e = Error::UnknownInputType {
        kind: "bogus".to_owned(),
        at: at(),
    };
    assert_eq!(
        e.to_string(),
        r#"In file aircraft/box.xml: line 12: no input type for <input type="bogus">"#
    );
    assert_eq!(e.location(), Some(&at()));
}

#[test]
fn wrapped_xml_and_function_errors_delegate() {
    let xml = Error::from(XmlError::NoData {
        element: "thing".to_owned(),
        at: at(),
    });
    assert_eq!(xml.location(), Some(&at()));
    let function = Error::from(FunctionError::NotBinary { at: at() });
    assert_eq!(function.location(), Some(&at()));
}

#[test]
fn errors_without_an_element_have_no_location() {
    let malformed = XmlError::Malformed {
        file: "aircraft/box.xml".to_owned(),
        line: 1,
        column: 2,
        message: "unclosed".to_owned(),
    };
    assert_eq!(malformed.location(), None);
    let encoding = XmlError::Encoding {
        file: "aircraft/box.xml".to_owned(),
        encoding: "EBCDIC".to_owned(),
    };
    assert_eq!(encoding.location(), None);
    assert_eq!(Error::from(malformed).location(), None);
    let unknown = FunctionError::UnknownId {
        what: "table",
        id: 3,
    };
    assert_eq!(unknown.location(), None);
    let recursion = FunctionError::Recursion {
        property: "a/b".to_owned(),
    };
    assert_eq!(recursion.location(), None);
    for e in [
        Error::ResourceNotFound {
            path: "x.xml".to_owned(),
        },
        Error::TrimFailed {
            message: "Trim Failed".to_owned(),
        },
        Error::Geographic {
            message: "Equatorial radius is not positive".to_owned(),
        },
        Error::ResourceUnreadable {
            path: "x.xml".to_owned(),
            kind: std::io::ErrorKind::PermissionDenied,
        },
    ] {
        assert_eq!(e.location(), None, "{e}");
    }
}

/// The location is the variant's alone: a message that also bakes it in
/// would display it twice.
#[test]
fn a_refused_load_names_its_place_once() {
    let mut files = MemoryResolver::new();
    files.insert(
        "aircraft/x/x.xml",
        r#"<fdm_config name="x" version="2.0"/>"#,
    );
    files.insert("aircraft/x/reset.xml", "<reset/>");
    files.insert("script.xml", r#"<runscript name="s"/>"#);
    let fdm = || Fdm::new().unwrap();
    for e in [
        fdm().load_model(&files, "aircraft", "x").unwrap_err(),
        fdm().load_script(&files, "script.xml").unwrap_err(),
        fdm()
            .load_initial_conditions(&files, "aircraft/x", "reset")
            .unwrap_err(),
    ] {
        assert!(matches!(e, Error::Model { .. }), "{e:?}");
        let shown = e.to_string();
        assert_eq!(shown.matches("In file ").count(), 1, "{shown}");
        let at = e.location().unwrap();
        assert!(shown.starts_with(&at.to_string()), "{shown}");
    }
}
