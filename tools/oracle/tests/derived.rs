// SPDX-License-Identifier: LGPL-2.1-or-later
//! The aircraft in oracle/derived are what `oracle derive` makes from the
//! vendored upstream files.

use std::path::Path;

#[test]
fn derived_files_match_their_upstream_source() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let expected = oracle::derived::expected_blocking(&repo).unwrap();
    assert!(!expected.is_empty());
    for (rel, text) in expected {
        let path = repo.join("oracle").join("derived").join(&rel);
        let actual = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(actual == text, "{rel} is stale; run `oracle derive`");
    }
}
