// SPDX-License-Identifier: LGPL-2.1-or-later
//! Comparator self-test through the `oracle compare` binary on the committed
//! ball golden: the reference against itself passes, and a perturbed copy
//! fails naming the perturbed property and time.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use oracle::Series;

const PROPERTY: &str = "velocities/u-fps";
const ROW: usize = 500;

#[cfg(test)]
fn golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/goldens/ball/reference.csv")
}

#[cfg(test)]
fn run_compare(candidate: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oracle"))
        .env("NO_COLOR", "1")
        .args(["compare", "--case", "ball"])
        .arg(golden())
        .arg(candidate)
        .output()
        .unwrap()
}

#[cfg(test)]
fn perturbed(delta: impl Fn(f64) -> f64) -> (tempfile::TempDir, PathBuf, f64) {
    let mut series = Series::read_blocking(&golden()).unwrap();
    let column = series.columns.iter().position(|c| c == PROPERTY).unwrap();
    let time = series.rows[ROW][0];
    series.rows[ROW][column] = delta(series.rows[ROW][column]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("candidate.csv");
    series.write_blocking(&path).unwrap();
    (dir, path, time)
}

#[cfg(test)]
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn reference_against_itself_passes() {
    let output = run_compare(&golden());
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).contains("ball: PASS, 1081 rows x 14 columns"));
}

#[test]
fn perturbation_within_tolerance_passes() {
    let (_dir, path, _) = perturbed(|v| v + v.abs() * 1e-13);
    let output = run_compare(&path);
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn perturbed_copy_fails_at_the_perturbed_property_and_time() {
    let (_dir, path, time) = perturbed(|v| v + 1.0);
    let output = run_compare(&path);
    assert_eq!(output.status.code(), Some(1));
    let text = stderr(&output);
    assert!(
        text.contains(&format!("FIRST DIVERGENCE t={time} {PROPERTY}:")),
        "{text}"
    );
}

#[test]
fn nan_in_candidate_fails() {
    let (_dir, path, time) = perturbed(|_| f64::NAN);
    let output = run_compare(&path);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains(&format!("t={time} {PROPERTY}: reference=")));
}

#[test]
fn unknown_case_is_an_operational_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_oracle"))
        .env("NO_COLOR", "1")
        .args(["compare", "--case", "nope"])
        .arg(golden())
        .arg(golden())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("no tolerance row covers Time"));
}
