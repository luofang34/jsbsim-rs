// SPDX-License-Identifier: LGPL-2.1-or-later
//! `Fdm::snapshot_into` and `Fdm::restore` give back the run they saved:
//! every run of `oracle::steady_state`, the derived and planet tripods and
//! every corpus aircraft, flown from `oracle::bench::AIRBORNE_IC` as
//! tools/oracle/tests/zero_alloc.rs flies them, continues after a restore
//! bit for bit as it did after the snapshot (see `oracle::snapshot_ab`),
//! with the c172x's sensor noise and every turbulence model on, on a
//! ground callback, with a trim or reset requested and pending, and
//! across property nodes created while running. A snapshot of another
//! model is refused with the fingerprint part that differs.

use std::path::{Path, PathBuf};

use jsbsim::fdm::Fdm;
use jsbsim::snapshot::{FingerprintPart, Snapshot, SnapshotError};
use jsbsim::{Error, FileResolver, Resolver};
use oracle::snapshot_ab::{AbOutcome, ab, ab_with};
use oracle::steady_state::{
    FRAMES, PLANET_TRIPOD_ALTITUDE_FT, PLANET_TRIPOD_DIR, PLANET_TRIPODS, SCRIPTS, ScriptRun,
    TRIMMED,
};

/// Frames each trace records.
const AFTER: u32 = 120;

/// Frames zero_alloc runs before it measures.
const WARM_UP: u32 = 100;

#[cfg(test)]
fn oracle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

/// Runs up to `frames` frames; false if the run ended.
#[cfg(test)]
fn run(fdm: &mut Fdm, frames: u32) -> bool {
    (0..frames).all(|_| fdm.run().unwrap())
}

/// Runs until the simulation time reaches `t`.
#[cfg(test)]
fn run_to(fdm: &mut Fdm, t: f64) {
    while fdm.get_property("simulation/sim-time-sec").unwrap() < t {
        assert!(fdm.run().unwrap(), "the run ended before {t} s");
    }
}

#[cfg(test)]
fn check(what: &str, outcome: &AbOutcome) {
    assert!(outcome.frames > 0, "{what}: no frame recorded");
    assert_eq!(outcome.mismatch, None, "{what}: A and B differ");
    assert!(
        outcome.resnapshot_identical,
        "{what}: a snapshot after restoring differs from the one restored"
    );
}

#[cfg(test)]
fn script(spec: &ScriptRun) -> Fdm {
    let mut fdm = spec.load(&oracle_dir()).unwrap();
    fdm.run_ic().unwrap();
    fdm
}

#[cfg(test)]
fn spec(name: &str) -> &'static ScriptRun {
    SCRIPTS
        .iter()
        .chain(TRIMMED)
        .find(|s| s.script == name)
        .unwrap()
}

/// Snapshots at the start of each run's measured window, in its middle
/// and at its end, checking A against B at each and against B on a
/// freshly loaded executive at the first.
#[test]
fn every_steady_state_script_continues_bit_for_bit() {
    for spec in SCRIPTS.iter().chain(TRIMMED) {
        let mut fdm = script(spec);
        let mut fresh = script(spec);
        let end = WARM_UP + spec.frames;
        let mut at = 0;
        for (i, checkpoint) in [WARM_UP, WARM_UP + spec.frames / 2, end].iter().enumerate() {
            if !run(&mut fdm, checkpoint.saturating_sub(at)) {
                break;
            }
            at = *checkpoint;
            let what = format!("{} at frame {at}", spec.script);
            let into = (i == 0).then_some(&mut fresh);
            let outcome = ab_with(&mut fdm, into, AFTER, &mut |_| Ok(())).unwrap();
            check(&what, &outcome);
            at += u32::try_from(outcome.frames).unwrap();
        }
    }
}

/// The ground callback is the host's: a restore leaves it installed, and
/// the run continues on it.
#[test]
fn a_restore_keeps_the_ground_callback() {
    let spec = SCRIPTS
        .iter()
        .find(|s| s.ground_callback.is_some())
        .unwrap();
    let mut fdm = script(spec);
    let mut fresh = script(spec);
    run(&mut fdm, 300);
    check(
        spec.script,
        &ab_with(&mut fdm, Some(&mut fresh), AFTER, &mut |_| Ok(())).unwrap(),
    );
    assert!(fdm.models.inertial.has_custom_ground_callback());
    assert!(fresh.models.inertial.has_custom_ground_callback());
}

/// Snapshots while an event's trim waits out its delay (737_cruise trims
/// 5 s after 0.1 s), just before a trim fires (c172_cruise_8K at 1 s),
/// and during the undock burn ramps of ball_orbit_phase.
#[test]
fn pending_trims_and_running_ramps_continue_bit_for_bit() {
    for (name, t, frames) in [
        ("scripts/737_cruise.xml", 3.0, 300),
        ("scripts/c172_cruise_8K.xml", 0.9, AFTER),
        ("scripts/ball_orbit_phase.xml", 10.2, 300),
    ] {
        let mut fdm = script(spec(name));
        run_to(&mut fdm, t);
        check(&format!("{name} at {t} s"), &ab(&mut fdm, frames).unwrap());
    }
}

/// The c172x in cruise, its sensors noisy, in each turbulence model; in
/// the last, the winds draw from a generator of their own.
#[test]
fn turbulence_continues_bit_for_bit() {
    for turb_type in 1..=4 {
        let mut fdm = script(spec("scripts/c172_cruise_8K.xml"));
        run_to(&mut fdm, 2.0);
        for (property, value) in [
            ("atmosphere/turb-type", f64::from(turb_type)),
            ("atmosphere/turb-gain", 1.0),
            ("atmosphere/turb-rate", 3.0),
            (
                "atmosphere/turbulence/milspec/windspeed_at_20ft_AGL-fps",
                25.0,
            ),
            ("atmosphere/turbulence/milspec/severity", 4.0),
        ] {
            fdm.set_property(property, value).unwrap();
        }
        if turb_type == 4 {
            fdm.set_property("atmosphere/randomseed", 7.0).unwrap();
        }
        run(&mut fdm, 30);
        check(
            &format!("turbulence type {turb_type}"),
            &ab(&mut fdm, AFTER).unwrap(),
        );
    }
}

#[test]
fn planet_tripods_continue_bit_for_bit() {
    let resolver = FileResolver::new(oracle_dir());
    for name in PLANET_TRIPODS {
        let mut fdm = Fdm::new().unwrap();
        fdm.load_model(&resolver, PLANET_TRIPOD_DIR, name).unwrap();
        fdm.set_property("ic/h-sl-ft", PLANET_TRIPOD_ALTITUDE_FT)
            .unwrap();
        fdm.run_ic().unwrap();
        run(&mut fdm, WARM_UP);
        check(name, &ab(&mut fdm, AFTER).unwrap());
    }
}

#[test]
fn derived_tripods_continue_bit_for_bit() {
    let resolver = FileResolver::new(oracle_dir());
    for system in oracle::derived::tripod::SYSTEMS.iter().filter(|s| s.loads) {
        let name = format!("tripod_{}", system.suffix);
        let mut fdm = Fdm::new().unwrap();
        fdm.set_systems_path(system.systems_path);
        fdm.load_model(&resolver, "derived/aircraft", &name)
            .unwrap();
        fdm.set_property("ic/h-sl-ft", 10000.0).unwrap();
        fdm.run_ic().unwrap();
        // The sensor then draws its noise from a generator of its own.
        if name == "tripod_sensorrandomseed" {
            fdm.set_property("aero/sensor/qbar/randomseed", 5.0)
                .unwrap();
        }
        run(&mut fdm, WARM_UP);
        check(&name, &ab(&mut fdm, AFTER).unwrap());
    }
}

/// Aircraft whose recorded load outcome is `ok`, and can run.
#[cfg(test)]
fn corpus() -> Vec<String> {
    let text = std::fs::read_to_string(oracle_dir().join("model-loading.txt")).unwrap();
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once(' '))
        .filter(|(_, outcome)| outcome.trim() == "ok")
        .map(|(name, _)| name.to_owned())
        .filter(|name| !oracle::bench::UNRUNNABLE.iter().any(|(n, _)| n == name))
        .collect()
}

#[test]
fn corpus_aircraft_continue_bit_for_bit() {
    let resolver = FileResolver::new(oracle_dir().join("inputs"));
    let aircraft = corpus();
    assert!(!aircraft.is_empty());
    for name in &aircraft {
        let mut fdm = Fdm::new().unwrap();
        fdm.load_model(&resolver, "aircraft", name).unwrap();
        for (property, value) in oracle::bench::AIRBORNE_IC {
            fdm.set_property(property, value).unwrap();
        }
        fdm.run_ic().unwrap();
        run(&mut fdm, WARM_UP);
        check(name, &ab(&mut fdm, AFTER.min(FRAMES)).unwrap());
    }
}

/// Serves one script from memory and everything else from the oracle's
/// inputs.
#[cfg(test)]
struct Overlay {
    files: FileResolver,
    script: &'static str,
}

#[cfg(test)]
impl Resolver for Overlay {
    fn resolve(&self, path: &str) -> Result<Vec<u8>, Error> {
        match path {
            OVERLAY_SCRIPT => Ok(self.script.as_bytes().to_vec()),
            _ => self.files.resolve(path),
        }
    }

    fn exists(&self, path: &str) -> bool {
        path == OVERLAY_SCRIPT || self.files.exists(path)
    }
}

#[cfg(test)]
const OVERLAY_SCRIPT: &str = "scripts/p67_snapshot.xml";

/// The c172x in cruise. At 0.5 s an event resets to the initial
/// conditions 0.25 s later, once: `test/armed` is persistent, so the
/// reset leaves it cleared. At 1.0 s another copies `test/late`, which
/// only the host creates (at 0.6 s), into `test/out`, binding its
/// function's operand late.
#[cfg(test)]
const SCRIPT: &str = r#"<?xml version="1.0"?>
<runscript name="snapshot test">
  <use aircraft="c172x" initialize="reset01"/>
  <run start="0.0" end="30" dt="0.0083333">
    <property value="1" persistent="true">test/armed</property>
    <property value="0">test/out</property>
    <event name="reset later">
      <condition>
        simulation/sim-time-sec ge 0.5
        test/armed eq 1
      </condition>
      <delay>0.25</delay>
      <set name="test/armed" value="0"/>
      <set name="simulation/reset" value="0"/>
    </event>
    <event name="copy late">
      <condition>simulation/sim-time-sec ge 1.0</condition>
      <set name="test/out">
        <function><product><property>test/late</property><value>2</value></product></function>
      </set>
    </event>
  </run>
</runscript>
"#;

#[cfg(test)]
fn overlay_fdm() -> Fdm {
    let overlay = Overlay {
        files: FileResolver::new(oracle_dir().join("inputs")),
        script: SCRIPT,
    };
    let mut fdm = Fdm::new().unwrap();
    fdm.load_script(&overlay, OVERLAY_SCRIPT).unwrap();
    fdm.run_ic().unwrap();
    fdm
}

/// The host creates `test/late` once 0.6 s have passed, as it would
/// again after a rewind; before it, another node, so that after a
/// restore to before both the late node lands on another slot than it
/// had in the run snapshotted, unless the restore dropped both.
#[cfg(test)]
fn host(fdm: &mut Fdm) -> Result<(), Error> {
    let t = fdm.get_property("simulation/sim-time-sec")?;
    if t >= 0.6 && fdm.node("test/late")?.is_none() {
        fdm.set_property("test/other", 1.0)?;
        fdm.set_property("test/late", 21.0)?;
    }
    Ok(())
}

/// Snapshots before the reset is due, while it waits its delay, before
/// and after the host creates `test/late`, and after the event bound to
/// it; each restored both in place and on a freshly loaded executive.
#[test]
fn requests_and_runtime_properties_continue_bit_for_bit() {
    for t in [0.3, 0.55, 0.65, 1.2] {
        let mut fdm = overlay_fdm();
        let mut fresh = overlay_fdm();
        while fdm.get_property("simulation/sim-time-sec").unwrap() < t {
            host(&mut fdm).unwrap();
            assert!(fdm.run().unwrap());
        }
        let created = fdm.node("test/late").unwrap().is_some();
        assert_eq!(created, t > 0.6, "test/late exists at {t} s: {created}");
        check(
            &format!("p67 script at {t} s, in place"),
            &ab_with(&mut fdm, None, 400, &mut host).unwrap(),
        );
        let mut again = overlay_fdm();
        while again.get_property("simulation/sim-time-sec").unwrap() < t {
            host(&mut again).unwrap();
            assert!(again.run().unwrap());
        }
        check(
            &format!("p67 script at {t} s, on a fresh executive"),
            &ab_with(&mut again, Some(&mut fresh), 400, &mut host).unwrap(),
        );
        assert_eq!(fresh.get_property("test/out").unwrap(), 42.0);
        assert_eq!(fresh.get_property("test/armed").unwrap(), 0.0);
    }
}

/// A restore to before the nodes created while running drops them, so a
/// different future can create others in their slots, and the operands
/// late-bound to the dropped ones bind again by name.
#[test]
fn a_rewind_past_created_nodes_rebinds_by_name() {
    let mut fdm = overlay_fdm();
    run_to(&mut fdm, 0.3);
    let mut early = Snapshot::new();
    fdm.snapshot_into(&mut early).unwrap();
    while fdm.get_property("simulation/sim-time-sec").unwrap() < 1.2 {
        host(&mut fdm).unwrap();
        assert!(fdm.run().unwrap());
    }
    assert_eq!(fdm.get_property("test/out").unwrap(), 42.0);
    fdm.restore(&early).unwrap();
    assert!(fdm.node("test/late").unwrap().is_none());
    assert!(fdm.node("test/other").unwrap().is_none());
    // Another future: test/late is created first, and with another value.
    run_to(&mut fdm, 0.6);
    fdm.set_property("test/late", 5.0).unwrap();
    fdm.set_property("test/other", 1.0).unwrap();
    run_to(&mut fdm, 1.2);
    assert_eq!(fdm.get_property("test/out").unwrap(), 10.0);
    assert_eq!(fdm.get_property("test/other").unwrap(), 1.0);
}

/// Serves the oracle's inputs with the ball's aircraft file edited: its
/// empty weight 20000 lbs becomes 20001, one byte.
#[cfg(test)]
struct EditedBall {
    files: FileResolver,
}

#[cfg(test)]
const BALL_AIRCRAFT: &str = "aircraft/ball/ball.xml";

#[cfg(test)]
impl Resolver for EditedBall {
    fn resolve(&self, path: &str) -> Result<Vec<u8>, Error> {
        let bytes = self.files.resolve(path)?;
        if path != BALL_AIRCRAFT {
            return Ok(bytes);
        }
        let text = String::from_utf8(bytes).unwrap();
        let original = r#"<emptywt unit="LBS"> 20000 </emptywt>"#;
        assert!(text.contains(original), "{BALL_AIRCRAFT} has no {original}");
        let edited = r#"<emptywt unit="LBS"> 20001 </emptywt>"#;
        Ok(text.replacen(original, edited, 1).into_bytes())
    }

    fn exists(&self, path: &str) -> bool {
        self.files.exists(path)
    }
}

/// `script` loaded from `resolver` and initialised.
#[cfg(test)]
fn loaded(resolver: &dyn Resolver, script: &str) -> Fdm {
    let mut fdm = Fdm::new().unwrap();
    fdm.load_script(resolver, script).unwrap();
    fdm.run_ic().unwrap();
    fdm
}

#[cfg(test)]
fn mismatch(part: FingerprintPart) -> Result<(), Error> {
    Err(Error::Snapshot(SnapshotError::FingerprintMismatch { part }))
}

/// A snapshot of one model does not restore onto another: the error
/// names the first part of the fingerprint that differs, and since the
/// header is checked before anything is written, the executive it was
/// refused by is as it was and still restores its own snapshots.
#[test]
fn a_snapshot_of_another_model_is_refused_untouched() {
    let inputs = FileResolver::new(oracle_dir().join("inputs"));
    let mut ball = loaded(&inputs, "scripts/ball.xml");
    run(&mut ball, 50);
    let mut ball_snapshot = Snapshot::new();
    ball.snapshot_into(&mut ball_snapshot).unwrap();

    let mut mk82 = loaded(&inputs, "scripts/mk82_script.xml");
    run(&mut mk82, 50);
    let mut before = Snapshot::new();
    mk82.snapshot_into(&mut before).unwrap();
    assert_eq!(
        mk82.restore(&ball_snapshot),
        mismatch(FingerprintPart::Documents)
    );
    let mut after = Snapshot::new();
    mk82.snapshot_into(&mut after).unwrap();
    assert!(after == before, "the refused restore changed the state");
    check(
        "mk82 after a refused restore",
        &ab(&mut mk82, AFTER).unwrap(),
    );

    // The same aircraft with one byte of its file changed differs in its
    // documents alone.
    let edited = EditedBall { files: inputs };
    let mut edited_ball = loaded(&edited, "scripts/ball.xml");
    let (ours, theirs) = (
        edited_ball.fingerprint().unwrap(),
        ball_snapshot.fingerprint().unwrap(),
    );
    assert_ne!(ours.documents, theirs.documents);
    assert_eq!(
        (ours.tree, ours.load_nodes),
        (theirs.tree, theirs.load_nodes)
    );
    assert_eq!(ours.models, theirs.models);
    assert_eq!(
        edited_ball.restore(&ball_snapshot),
        mismatch(FingerprintPart::Documents)
    );

    // The same documents loaded after the host created a node differ in
    // the load-time tree.
    let mut extra = Fdm::new().unwrap();
    extra.set_property("test/before-load", 1.0).unwrap();
    extra
        .load_script(
            &FileResolver::new(oracle_dir().join("inputs")),
            "scripts/ball.xml",
        )
        .unwrap();
    extra.run_ic().unwrap();
    assert_eq!(extra.fingerprint().unwrap().documents, theirs.documents);
    assert_eq!(
        extra.restore(&ball_snapshot),
        mismatch(FingerprintPart::Tree)
    );

    ball.restore(&ball_snapshot).unwrap();
}
