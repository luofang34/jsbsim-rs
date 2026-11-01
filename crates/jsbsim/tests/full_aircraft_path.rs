// SPDX-License-Identifier: LGPL-2.1-or-later
//! `Fdm::full_aircraft_path` after each way of loading an aircraft.

use jsbsim::{Error, Fdm, MemoryResolver};

const AIRCRAFT: &str = r#"<fdm_config name="box">
  <metrics/>
  <mass_balance>
    <ixx>10</ixx><iyy>10</iyy><izz>10</izz><emptywt>100</emptywt>
    <location name="CG" unit="IN"><x>0</x><y>0</y><z>0</z></location>
  </mass_balance>
  <ground_reactions/>
</fdm_config>"#;

#[cfg(test)]
fn files() -> MemoryResolver {
    let mut files = MemoryResolver::new();
    files.insert("aircraft/box/box.xml", AIRCRAFT);
    files.insert(
        "aircraft/box/ic.xml",
        r#"<initialize name="ic"><altitude unit="FT">1000</altitude></initialize>"#,
    );
    files.insert("models/Odd/plane.v2.xml", AIRCRAFT);
    files.insert("loose.xml", AIRCRAFT);
    files.insert(
        "scripts/box.xml",
        r#"<runscript name="box"><use aircraft="box" initialize="ic"/>
             <run start="0" end="1" dt="0.01"/></runscript>"#,
    );
    files
}

#[test]
fn empty_before_a_model_loads() {
    assert_eq!(Fdm::new().unwrap().full_aircraft_path(), "");
}

#[test]
fn a_model_document_not_found_leaves_it_empty() {
    let mut fdm = Fdm::new().unwrap();
    let missing = fdm.load_model(&files(), "aircraft", "missing");
    assert!(
        matches!(missing, Err(Error::ResourceNotFound { .. })),
        "{missing:?}"
    );
    assert_eq!(fdm.full_aircraft_path(), "");
}

#[test]
fn load_model_gives_the_directory_named_after_the_model() {
    let mut fdm = Fdm::new().unwrap();
    fdm.load_model(&files(), "aircraft", "box").unwrap();
    assert_eq!(fdm.full_aircraft_path(), "aircraft/box");
    assert!(format!("{fdm:?}").contains(r#"aircraft: "box""#), "{fdm:?}");
}

#[test]
fn load_model_file_gives_the_directory_of_the_file() {
    for (path, dir) in [("models/Odd/plane.v2", "models/Odd"), ("loose.xml", ".")] {
        let mut fdm = Fdm::new().unwrap();
        fdm.load_model_file(&files(), path).unwrap();
        assert_eq!(fdm.full_aircraft_path(), dir, "{path}");
    }
}

#[test]
fn load_script_gives_the_directory_of_the_aircraft_it_uses() {
    let mut fdm = Fdm::new().unwrap();
    fdm.load_script(&files(), "scripts/box.xml").unwrap();
    assert_eq!(fdm.full_aircraft_path(), "aircraft/box");
}
