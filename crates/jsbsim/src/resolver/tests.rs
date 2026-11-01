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

/// Something at the path that cannot be read is not reported as missing.
#[cfg(feature = "fs")]
#[test]
fn file_resolver_tells_missing_from_unreadable() {
    let resolver = FileResolver::new(env!("CARGO_MANIFEST_DIR"));
    assert!(!resolver.resolve("Cargo.toml").unwrap().is_empty());
    assert_eq!(
        resolver.resolve("none.xml").unwrap_err(),
        Error::ResourceNotFound {
            path: "none.xml".to_owned()
        }
    );
    let err = resolver.resolve("src").unwrap_err();
    assert!(
        matches!(&err, Error::ResourceUnreadable { path, kind }
            if path == "src" && *kind != std::io::ErrorKind::NotFound),
        "{err:?}"
    );
    assert!(err.to_string().starts_with("cannot read src: "), "{err}");
}
