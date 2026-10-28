// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

#[test]
fn timing_line_is_found_among_console_messages() {
    let stdout = "In file ball.xml: line 134\n\ndt=0.01 every_n_frames=1000\nmore\n";
    assert_eq!(parse_timing(stdout).unwrap(), (0.01, 1000));
}

#[test]
fn missing_or_malformed_timing_is_an_error() {
    assert!(parse_timing("nothing here\n").is_err());
    assert!(parse_timing("dt=x every_n_frames=1\n").is_err());
    assert!(parse_timing("dt=0.1\n").is_err());
}

#[test]
fn paths_stay_under_target() {
    let paths = Paths {
        repo_root: PathBuf::from("/r"),
        jsbsim_root: PathBuf::from("/j"),
    };
    assert!(paths.build_dir().starts_with("/r/target"));
    assert!(paths.driver_exe().starts_with("/r/target"));
    assert_eq!(paths.goldens_dir(), Path::new("/r/oracle/goldens"));
}
