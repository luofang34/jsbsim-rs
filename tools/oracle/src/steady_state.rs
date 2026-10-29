// SPDX-License-Identifier: LGPL-2.1-or-later
//! The runs whose steady-state frames tools/oracle/tests/zero_alloc.rs and
//! tools/oracle/tests/derived_cache.rs both measure, beside the corpus
//! aircraft of `oracle/model-loading.txt`. One list serves both tests so
//! that neither can lose an entry the other keeps; the unit tests require
//! each script to be a golden's and every engine, thruster, buoyancy and
//! atmosphere family to be flown by at least one run.

#[cfg(test)]
mod tests;

/// A script flown from its initial conditions through its events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptRun {
    /// Directory under `oracle/` the script and its aircraft resolve in.
    pub root: &'static str,
    /// The script, relative to `root`.
    pub script: &'static str,
    /// Frames measured after the warm-up.
    pub frames: u32,
}

const fn run(root: &'static str, script: &'static str, frames: u32) -> ScriptRun {
    ScriptRun {
        root,
        script,
        frames,
    }
}

/// Frames measured for a run with no window of its own to cover.
pub const FRAMES: u32 = 1000;

/// The scripts measured from their first frame after the warm-up.
///
/// ball_orbit_phase's window covers its undock burn ramps at 10 s and
/// 12 s; mk82's ends before touchdown; gear_rover_taxi's rolls, steers,
/// brakes and rests on every kind of contact, and gear_rover_retract's
/// slides on its skids with the gear up and lands back on its wheels;
/// piston_start_c310's cranks, starts, runs on each magneto, cuts off and
/// restarts its engines, and piston_governor_c310's governs, feathers,
/// windmills and restarts in flight; c1722_inertia's trims, starts running
/// from its initial conditions and burns its tanks down to the unusable
/// fuel; f16_test's turbine spins up, lights and spools to full throttle;
/// L4102's turboprops start running from their initial conditions;
/// Test_F450_Launch's brushless motors lift the quadcopter off; J2461's
/// main engines start and its solid boosters ignite and burn; x153's
/// XLR-99 lights and throttles up; sim_primer's engine spins the ah1s's
/// rotors up through the transmission and its governor takes over; and
/// the airships' and the weather balloon's gas cells, ballonets and their
/// heat and blower functions run in Submarine_Scout_1, ZLT-NT-moored-1
/// and weather-balloon.
pub const SCRIPTS: &[ScriptRun] = &[
    run("inputs", "scripts/ball_orbit_phase.xml", 3000),
    run("inputs", "scripts/ball.xml", FRAMES),
    run("inputs", "scripts/mk82_script.xml", FRAMES),
    run("inputs", "scripts/c1724.xml", FRAMES),
    run("scenarios", "scripts/gear_rover_taxi.xml", 2000),
    run("scenarios", "scripts/gear_rover_retract.xml", 1000),
    run("derived", "scripts/piston_start_c310.xml", 3000),
    run("derived", "scripts/piston_governor_c310.xml", 5000),
    run("derived", "scripts/c1722_inertia.xml", 12000),
    run("inputs", "scripts/f16_test.xml", 3000),
    run("inputs", "scripts/L4102.xml", 3000),
    run("inputs", "scripts/Test_F450_Launch.xml", 3000),
    run("inputs", "scripts/J2461.xml", 2400),
    run("inputs", "scripts/x153.xml", 3000),
    run("inputs", "scripts/sim_primer.xml", 3000),
    run("inputs", "scripts/Submarine_Scout_1.xml", 3000),
    run("inputs", "scripts/ZLT-NT-moored-1.xml", 3000),
    run("inputs", "scripts/weather-balloon.xml", 3000),
];

/// The scripts that trim through `simulation/do_simple_trim`, measured
/// from their first frame after the trim has completed: a trim allocates
/// its axes and contacts when it runs, as FGTrim does.
pub const TRIMMED: &[ScriptRun] = &[
    run("inputs", "scripts/c1722.xml", FRAMES),
    run("inputs", "scripts/c172_cruise_8K.xml", FRAMES),
    run("inputs", "scripts/c172_cross_wind.xml", FRAMES),
    run("inputs", "scripts/c172_head_wind.xml", FRAMES),
    run("inputs", "scripts/c172_elevator_doublet.xml", FRAMES),
    run("inputs", "scripts/p51d_tail_wind.xml", FRAMES),
    run("inputs", "scripts/Short_S23_2.xml", FRAMES),
    run("inputs", "scripts/Short_S23_3.xml", FRAMES),
    run("inputs", "scripts/Short_S23_4.xml", FRAMES),
    run("inputs", "scripts/737_cruise.xml", FRAMES),
];

/// Directory under `oracle/` of the planet tripods.
pub const PLANET_TRIPOD_DIR: &str = "derived/aircraft";

/// The tripods flown in the Martian and NRLMSISE-00 atmospheres, from
/// `ic/h-sl-ft` [`PLANET_TRIPOD_ALTITUDE_FT`] for [`FRAMES`] frames.
pub const PLANET_TRIPODS: &[&str] = &["tripod_mars", "tripod_msis"];

/// The planet tripods' initial altitude, ft.
pub const PLANET_TRIPOD_ALTITUDE_FT: f64 = 10000.0;
