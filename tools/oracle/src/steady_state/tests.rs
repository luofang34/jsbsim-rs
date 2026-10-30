// SPDX-License-Identifier: LGPL-2.1-or-later
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use jsbsim::Fdm;
use jsbsim::FileResolver;
use jsbsim::oracle_support::models::atmosphere::AtmosphereModel;
use jsbsim::oracle_support::models::propulsion::families::{
    REGISTERED_ENGINES, REGISTERED_THRUSTERS,
};
use jsbsim::oracle_support::xml::DocumentLoader;

use super::*;
use crate::manifest::{self, CaseSpec, Manifest};

fn oracle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

fn all_scripts() -> impl Iterator<Item = &'static ScriptRun> {
    SCRIPTS.iter().chain(TRIMMED)
}

/// Aircraft whose recorded load outcome is `ok`.
fn loading_aircraft() -> BTreeSet<String> {
    let text = std::fs::read_to_string(oracle_dir().join("model-loading.txt")).unwrap();
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once(' '))
        .filter(|(_, outcome)| outcome.trim() == "ok")
        .map(|(name, _)| name.to_owned())
        .collect()
}

/// The `<use aircraft>` of `run`'s script.
fn script_aircraft(run: &ScriptRun) -> String {
    let resolver = FileResolver::new(oracle_dir().join(run.root));
    let script = DocumentLoader::new(&resolver, "aircraft")
        .load_document(run.script)
        .unwrap();
    let used = script.find_element("use").unwrap();
    used.attribute("aircraft").to_owned()
}

/// (script, inputs root under `oracle/`, ground callback) of every script
/// golden.
fn golden_scripts() -> BTreeSet<(String, String, Option<String>)> {
    let goldens = oracle_dir().join("goldens");
    let mut scripts = BTreeSet::new();
    for case in manifest::list_cases_blocking(&goldens).unwrap() {
        let manifest = Manifest::read_blocking(&manifest::case_dir(&goldens, &case)).unwrap();
        if let CaseSpec::Script(spec) = manifest.case {
            let root = spec.inputs.as_deref().unwrap_or("oracle/inputs");
            if let Some(root) = root.strip_prefix("oracle/") {
                scripts.insert((spec.script, root.to_owned(), spec.ground_callback));
            }
        }
    }
    scripts
}

#[test]
fn every_run_is_a_golden_script_of_an_aircraft_that_loads() {
    let goldens = golden_scripts();
    let loading = loading_aircraft();
    let mut seen = BTreeSet::new();
    for run in all_scripts() {
        assert!(
            seen.insert((run.root, run.script)),
            "{run:?} is listed twice"
        );
        let key = (
            run.script.to_owned(),
            run.root.to_owned(),
            run.ground_callback.map(str::to_owned),
        );
        assert!(goldens.contains(&key), "{run:?} is no golden's script");
        // The derived and scenario aircraft are not corpus aircraft.
        if run.root == "inputs" {
            let aircraft = script_aircraft(run);
            assert!(
                loading.contains(&aircraft),
                "{run:?} flies {aircraft}, which oracle/model-loading.txt does not record as ok"
            );
        }
    }
}

/// What the loaded runs fly.
#[derive(Default)]
struct Flown {
    engines: BTreeSet<&'static str>,
    thrusters: BTreeSet<&'static str>,
    gas_cells: bool,
    ballonets: bool,
    atmospheres: Vec<AtmosphereModel>,
}

impl Flown {
    fn add(&mut self, fdm: &Fdm) {
        for engine in &fdm.models.propulsion.engines {
            self.engines.insert(engine.kind.element());
            self.thrusters.insert(engine.thruster.element());
        }
        let cells = &fdm.models.buoyant_forces.cells;
        self.gas_cells |= !cells.is_empty();
        self.ballonets |= cells.iter().any(|c| !c.ballonets.is_empty());
        self.atmospheres.push(fdm.models.atmosphere.model);
    }
}

#[test]
fn every_family_is_flown_by_a_listed_run() {
    let mut flown = Flown::default();
    for run in all_scripts() {
        flown.add(&run.load(&oracle_dir()).unwrap());
    }
    let resolver = FileResolver::new(oracle_dir());
    for name in PLANET_TRIPODS {
        let mut fdm = Fdm::new().unwrap();
        fdm.load_model(&resolver, PLANET_TRIPOD_DIR, name).unwrap();
        flown.add(&fdm);
    }
    for element in REGISTERED_ENGINES {
        assert!(flown.engines.contains(element), "no run flies a {element}");
    }
    for element in REGISTERED_THRUSTERS {
        assert!(
            flown.thrusters.contains(element),
            "no run flies a {element}"
        );
    }
    assert!(flown.gas_cells, "no run flies a gas cell");
    assert!(flown.ballonets, "no run flies a ballonet");
    for model in AtmosphereModel::ALL {
        assert!(
            flown.atmospheres.contains(&model),
            "no run flies the {model:?} atmosphere"
        );
    }
}
