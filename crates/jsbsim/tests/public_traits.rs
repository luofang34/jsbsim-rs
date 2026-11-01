// SPDX-License-Identifier: LGPL-2.1-or-later
//! Standard traits that hosts of the crate rely on: `#[derive]`s of their
//! own, `unwrap_err`, comparing errors and contacts, and map keys.

use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;

use jsbsim::error::SourceLocation;
use jsbsim::math::{ColumnVector3, Location};
use jsbsim::snapshot::{Fingerprint, FingerprintPart};
use jsbsim::trim::{Control, State, TrimMode};
use jsbsim::{Contact, Error, Fdm};

const fn assert_debug<T: Debug>() {}
const fn assert_eq_trait<T: Eq>() {}
const fn assert_partial_eq<T: PartialEq>() {}
const fn assert_hash<T: Hash>() {}

const _: () = {
    assert_debug::<Fdm>();
    assert_eq_trait::<Error>();
    assert_partial_eq::<Contact>();
    assert_hash::<Fingerprint>();
    assert_hash::<FingerprintPart>();
    assert_hash::<SourceLocation>();
    assert_hash::<TrimMode>();
    assert_hash::<Control>();
    assert_hash::<State>();
};

#[test]
fn a_host_derive_shows_the_executive_as_a_summary() {
    #[derive(Debug)]
    struct Host {
        fdm: Result<Fdm, Error>,
    }

    let host = Host { fdm: Fdm::new() };
    assert!(host.fdm.is_ok());
    let shown = format!("{host:?}");
    for field in ["aircraft: \"\"", "sim_time: 0.0", "frame: 0"] {
        assert!(shown.contains(field), "{field} in {shown}");
    }
    assert!(shown.contains("model_loaded: false"), "{shown}");
    assert!(shown.ends_with(".. }) }"), "{shown}");
}

#[test]
fn errors_compare_with_eq() {
    let missing = |path: &str| Error::ResourceNotFound {
        path: path.to_owned(),
    };
    assert_eq!(missing("a.xml"), missing("a.xml"));
    assert_ne!(missing("a.xml"), missing("b.xml"));
}

#[test]
fn contacts_compare_every_field() {
    let normal = ColumnVector3::new(0.0, 0.0, 1.0);
    let contact = Contact::new(10.0, Location::default(), normal);
    assert_eq!(contact, contact);
    assert_ne!(contact, Contact::new(11.0, Location::default(), normal));
    let moving = contact.with_motion(normal, ColumnVector3::zero());
    assert_ne!(contact, moving);
}

#[test]
fn trim_selectors_are_map_keys() {
    let modes = HashSet::from([TrimMode::Longitudinal, TrimMode::Full, TrimMode::Full]);
    assert_eq!(modes.len(), 2);
    let axes = HashSet::from([(State::Wdot, Control::Alpha), (State::Wdot, Control::Alpha)]);
    assert_eq!(axes.len(), 1);
}
