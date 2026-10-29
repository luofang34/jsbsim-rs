// SPDX-License-Identifier: LGPL-2.1-or-later
//! The snapshot byte layout is pinned by one golden stream: ball.xml's
//! snapshot 100 frames after its initial conditions, in
//! `oracle/snapshot/ball-v<FORMAT_VERSION>.bin`. A change to what a
//! snapshot stores or in which order fails here, so it cannot ship
//! without a new `jsbsim::snapshot::FORMAT_VERSION`.
//!
//! To change the layout deliberately: bump `FORMAT_VERSION`, delete the
//! old golden, run
//! `JSBSIM_WRITE_SNAPSHOT_GOLDEN=1 cargo test -p oracle --test snapshot_golden`
//! once to write the new one, and commit it.

use std::path::{Path, PathBuf};

use jsbsim::FileResolver;
use jsbsim::fdm::Fdm;
use jsbsim::snapshot::{FORMAT_VERSION, Snapshot};

#[cfg(test)]
fn oracle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

#[cfg(test)]
fn ball() -> Fdm {
    let mut fdm = Fdm::new().unwrap();
    fdm.load_script(
        &FileResolver::new(oracle_dir().join("inputs")),
        "scripts/ball.xml",
    )
    .unwrap();
    fdm.run_ic().unwrap();
    fdm
}

#[test]
fn ball_snapshot_bytes_are_pinned() {
    let mut fdm = ball();
    for _ in 0..100 {
        assert!(fdm.run().unwrap());
    }
    let mut snapshot = Snapshot::new();
    fdm.snapshot_into(&mut snapshot).unwrap();
    let golden = oracle_dir()
        .join("snapshot")
        .join(format!("ball-v{FORMAT_VERSION}.bin"));
    if std::env::var_os("JSBSIM_WRITE_SNAPSHOT_GOLDEN").is_some() {
        std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
        std::fs::write(&golden, snapshot.as_bytes()).unwrap();
    }
    let pinned = std::fs::read(&golden).unwrap_or_else(|e| {
        panic!(
            "{}: {e}; a new FORMAT_VERSION needs its golden written (see this file's header)",
            golden.display()
        )
    });
    assert!(
        pinned == snapshot.as_bytes(),
        "the snapshot layout changed without a new FORMAT_VERSION ({} bytes pinned, {} now)",
        pinned.len(),
        snapshot.as_bytes().len()
    );

    // The pinned stream restores onto a fresh ball and comes back as is.
    let mut fresh = ball();
    fresh
        .restore(&Snapshot::from_bytes(&pinned).unwrap())
        .unwrap();
    let mut again = Snapshot::new();
    fresh.snapshot_into(&mut again).unwrap();
    assert!(again.as_bytes() == pinned);
}
