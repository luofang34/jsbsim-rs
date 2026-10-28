// SPDX-License-Identifier: LGPL-2.1-or-later
use super::super::tests::{load_error, load_system};
use crate::{Error, LoadOptions};

#[test]
fn a_distributor_must_be_inclusive_or_exclusive() {
    let system = r#"<system><channel name="c"><distributor name="d"/></channel></system>"#;
    let err = load_error(system, LoadOptions::default());
    assert!(
        matches!(&err, Error::Model { message } if message.contains("but got \"\" instead")),
        "{err:?}"
    );
}

#[test]
fn a_missing_target_or_value_fails_the_run() {
    for (target, value) in [("test/missing", "1"), ("test/x", "test/missing")] {
        let system = format!(
            r#"<system><property>test/x</property><channel name="c">
  <distributor name="d" type="inclusive">
    <case> <property value="{value}"> {target} </property> </case>
  </distributor>
</channel></system>"#
        );
        let mut fdm = load_system(&system, LoadOptions::default()).unwrap();
        let err = fdm.run_ic().unwrap_err();
        assert!(
            matches!(&err, Error::Model { message } if *message == format!("{target} in distributor component is not known")),
            "{err:?}"
        );
    }
}

#[test]
fn a_distributor_sets_no_outputs() {
    let system = r#"<system><property>test/x</property><channel name="c">
  <distributor name="d" type="exclusive">
    <case> <property value="2"> test/x </property> </case>
    <output>test/out</output>
  </distributor>
</channel></system>"#;
    let mut fdm = load_system(system, LoadOptions::default()).unwrap();
    fdm.set_property("test/out", 5.0).unwrap();
    fdm.run_ic().unwrap();
    assert_eq!(fdm.get_property("test/x").unwrap(), 2.0);
    assert_eq!(fdm.get_property("test/out").unwrap(), 5.0);
    assert_eq!(fdm.get_property("fcs/d").unwrap(), 0.0);
}
