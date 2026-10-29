// SPDX-License-Identifier: LGPL-2.1-or-later
//! JSBSim's `tests/CheckMomentsUpdate.py` on the weather balloon: the
//! hydrogen's weight moves the CG, and the buoyancy's moment follows a
//! moved point mass in the very next frame. The upstream test also reads
//! the moment back from an output CSV file, which the core does not write.

use std::path::{Path, PathBuf};

use jsbsim::FileResolver;
use jsbsim::fdm::Fdm;

/// `mol2lbs`: the weight of one mole of hydrogen, lbs.
#[cfg(test)]
const MOL2LBS: f64 = 0.00013841 * 32.174049;

#[cfg(test)]
fn inputs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/inputs")
}

#[cfg(test)]
fn get(fdm: &mut Fdm, path: &str) -> f64 {
    fdm.get_property(path).unwrap()
}

/// `CheckCGPosition`: the CG is the radiosonde's share of the weight
/// times its location, the balloon and the gas sitting at the origin.
#[cfg(test)]
fn check_cg_position(fdm: &mut Fdm) {
    let weight = get(fdm, "inertia/weight-lbs");
    let empty_weight = get(fdm, "inertia/empty-weight-lbs");
    let contents = get(fdm, "buoyant_forces/gas-cell/contents-mol");
    let radiosonde = weight - empty_weight - contents * MOL2LBS;
    for axis in ["x", "y", "z"] {
        let cg = get(fdm, &format!("inertia/cg-{axis}-in"));
        let location = get(
            fdm,
            &format!("inertia/pointmass-location-{}-inches", axis.to_uppercase()),
        );
        let expected = location * radiosonde / weight;
        assert!((cg - expected).abs() < 1e-7, "{axis}: {cg} vs {expected}");
    }
}

#[test]
fn the_buoyancy_moment_follows_a_moved_point_mass() {
    let mut fdm = Fdm::new().unwrap();
    fdm.load_script(&FileResolver::new(inputs()), "scripts/weather-balloon.xml")
        .unwrap();
    fdm.run_ic().unwrap();
    check_cg_position(&mut fdm);
    let dt = get(&mut fdm, "simulation/dt");
    while fdm.run().unwrap() {
        if get(&mut fdm, "simulation/sim-time-sec") > 1.0 - 2.0 * dt {
            break;
        }
    }
    check_cg_position(&mut fdm);
    fdm.set_property("inertia/pointmass-location-X-inches", 5.0)
        .unwrap();
    fdm.run().unwrap();
    check_cg_position(&mut fdm);
    let fbx = get(&mut fdm, "forces/fbx-buoyancy-lbs");
    let fbz = get(&mut fdm, "forces/fbz-buoyancy-lbs");
    let cgx = get(&mut fdm, "inertia/cg-x-in") / 12.0;
    let cgz = get(&mut fdm, "inertia/cg-z-in") / 12.0;
    let mby = get(&mut fdm, "moments/m-buoyancy-lbsft");
    assert!(
        (fbx * cgz - fbz * cgx - mby).abs() < 1e-7,
        "Fbx*CGz-Fbz*CGx = {} and Mby = {mby} do not match",
        fbx * cgz - fbz * cgx
    );
    fdm.run().unwrap();
    check_cg_position(&mut fdm);
}
