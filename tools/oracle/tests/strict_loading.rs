// SPDX-License-Identifier: LGPL-2.1-or-later
//! Loading strictly refuses only what the port has not ported: the
//! `<input>` and `<output>` directives load and run as JSBSim's do, so
//! goldens whose aircraft carry them match their C++ references strict or
//! not.

use std::path::{Path, PathBuf};

use jsbsim::LoadOptions;
use oracle::manifest::{CaseSpec, REFERENCE_FILE};
use oracle::{Manifest, Series, ToleranceTable, compare};

#[cfg(test)]
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Runs golden `case` with `options` up to `end_time` (the script's end
/// when `None`) and compares it with the reference rows it covers.
#[cfg(test)]
fn matches_reference(case: &str, options: LoadOptions, end_time: Option<f64>) {
    let root = repo_root();
    let dir = root.join("oracle/goldens").join(case);
    let manifest = Manifest::read_blocking(&dir).unwrap();
    let CaseSpec::Script(mut spec) = manifest.case else {
        panic!("{case} is not a script case");
    };
    if end_time.is_some() {
        spec.end_time = end_time;
    }
    let (candidate, stop) =
        oracle::port_script::run_script_with(&spec.rust_root(&root), &spec, options);
    assert!(stop.is_none(), "{case} {options:?}: {}", stop.unwrap());
    let mut reference = Series::read_blocking(&dir.join(REFERENCE_FILE)).unwrap();
    assert!(candidate.rows.len() > 1, "{case}: no frames ran");
    if end_time.is_some() {
        reference.rows.truncate(candidate.rows.len());
    }
    assert_eq!(reference.rows.len(), candidate.rows.len(), "{case}");
    let table = ToleranceTable::read_blocking(&root.join("oracle/tolerances.toml")).unwrap();
    let outcome = compare(case, &reference, &candidate, &table).unwrap();
    assert!(
        outcome.divergence.is_none(),
        "{case} {options:?}: {:?}",
        outcome.divergence
    );
}

#[test]
fn the_input_golden_matches_strict_and_not() {
    for strict in [true, false] {
        matches_reference("tripod_input", LoadOptions { strict }, None);
    }
}

#[test]
fn aircraft_with_inputs_and_outputs_load_and_fly_strictly() {
    // The 737 carries a TCP and a QTJSBSIM <input> and the c172x three
    // <output>s.
    for case in ["737_cruise", "c1722"] {
        matches_reference(case, LoadOptions { strict: true }, Some(2.0));
    }
}
