// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

#[test]
fn returns_inserted_bytes() {
    let mut resolver = MemoryResolver::new();
    resolver.insert("aircraft/ball/ball.xml", b"<fdm_config/>".to_vec());
    assert_eq!(
        resolver.resolve("aircraft/ball/ball.xml").unwrap(),
        b"<fdm_config/>"
    );
}

#[test]
fn missing_path_names_the_path() {
    let err = MemoryResolver::new()
        .resolve("engine/none.xml")
        .unwrap_err();
    assert_eq!(
        err,
        Error::ResourceNotFound {
            path: "engine/none.xml".to_owned()
        }
    );
    assert_eq!(err.to_string(), "resource not found: engine/none.xml");
}

#[test]
fn usable_as_trait_object() {
    let mut resolver = MemoryResolver::new();
    resolver.insert("a", "b");
    let dynamic: &dyn Resolver = &resolver;
    assert_eq!(dynamic.resolve("a").unwrap(), b"b");
}
