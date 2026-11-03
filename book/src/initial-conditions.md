# Initial conditions and resets

Initial conditions say where the aircraft starts: position, attitude,
speed, wind. They are held apart from the running state until
[`run_ic`](api/jsbsim/struct.Fdm.html#method.run_ic) applies them.

## Through properties

Every initial condition is an `ic/...` property. Write them in any order,
then call `run_ic`:

| Property | Meaning |
|---|---|
| `ic/lat-geod-deg`, `ic/lat-gc-deg`, `ic/long-gc-deg` | geodetic or geocentric latitude, longitude |
| `ic/h-sl-ft`, `ic/h-agl-ft` | altitude above sea level, or above the terrain |
| `ic/terrain-elevation-ft` | terrain elevation, for the default terrain |
| `ic/psi-true-deg`, `ic/theta-deg`, `ic/phi-deg` | heading, pitch, roll |
| `ic/vc-kts`, `ic/vt-kts`, `ic/vg-kts`, `ic/mach` | calibrated, true or ground speed, Mach number |
| `ic/u-fps`, `ic/v-fps`, `ic/w-fps` | body-axis velocity |
| `ic/vn-fps`, `ic/ve-fps`, `ic/vd-fps` | north, east, down velocity |
| `ic/alpha-deg`, `ic/beta-deg`, `ic/gamma-deg`, `ic/roc-fpm` | angle of attack, sideslip, flight path angle, climb rate |
| `ic/p-rad_sec`, `ic/q-rad_sec`, `ic/r-rad_sec` | body rates |
| `ic/vw-mag-fps`, `ic/vw-dir-deg` | wind speed and the direction it blows from |
| `ic/targetNlf` | load factor for a [pull-up trim](trim.md) |

Most angles also have `-rad` forms. These are JSBSim's own initial
condition properties and they interact as JSBSim's do: setting a speed
keeps the others consistent with it, and the last of several ways of giving
the same quantity wins. The API documentation of
[`Fdm`](api/jsbsim/struct.Fdm.html) points at the C++ class each mirrors,
`FGInitialCondition`, whose documentation lists them all.

## Through an initialization file

JSBSim's initialization (reset) files load with
[`load_initial_conditions`](api/jsbsim/struct.Fdm.html#method.load_initial_conditions),
given a directory and a name, or
[`load_initial_conditions_file`](api/jsbsim/struct.Fdm.html#method.load_initial_conditions_file),
given one path. `.xml` is added to a name without an extension:

```rust
{{#include ../../crates/jsbsim/examples/properties.rs:airborne}}
```

Both versions of the format load. A version 1 file (no `version`
attribute, or `1.x`) holds flat elements: `latitude`, `longitude`,
`altitude`, `psi`, `vc`, `ubody` and the rest. A version 2 file holds
`<position>`, `<orientation>`, `<velocity>` and `<attitude_rate>`, each in a
frame such as `ECEF` or `local`. The file is applied on top of the initial
conditions already set; it does not clear them first.

Two details are JSBSim's, and the port keeps them:

- In a version 1 file `<altitude>` means height **above the terrain**, the
  same as `<altitudeAGL>`. Use `<altitudeMSL>` for height above sea level.
- When a file is inconsistent (a latitude beyond ±90°, a missing position),
  JSBSim logs it and loads the rest. The port also loads the rest, then
  returns the first inconsistency as
  [`Error::Model`](api/jsbsim/error/enum.Error.html#variant.Model) with its
  file and line, and logs any further ones.

`<running>` names an engine to start running at `run_ic` (`-1` for all of
them), and `<trim>` names a trim the file asks for. The core records the
trim but does not run it: read it with
[`trim_requested`](api/jsbsim/struct.Fdm.html#method.trim_requested) and
[run it yourself](trim.md#trims-an-initialization-file-asks-for).

## Applying them

[`run_ic`](api/jsbsim/struct.Fdm.html#method.run_ic) copies the initial
conditions into the models and runs them for two frames with the clock
held, so that every derived value (air data, forces, accelerations) is
consistent before the first `run`. Then it starts the engines marked
`<running>`. Simulation time does not advance.

Call `run_ic` again whenever you change initial conditions and want the
aircraft placed there.

## Resetting

[`reset_to_initial_conditions`](api/jsbsim/struct.Fdm.html#method.reset_to_initial_conditions)
reinitializes every model, sets the clock back to zero (or to the script's
start time) and calls `run_ic`. With a script loaded it also re-arms the
script's events and returns its non-persistent properties to their declared
values. A [`ResetMask`](api/jsbsim/struct.ResetMask.html) modifies it:

| Flag | Effect |
|---|---|
| [`DONT_EXECUTE_RUN_IC`](api/jsbsim/struct.ResetMask.html#associatedconstant.DONT_EXECUTE_RUN_IC) | reinitialize, but leave `run_ic` to you |
| [`START_NEW_OUTPUT`](api/jsbsim/struct.ResetMask.html#associatedconstant.START_NEW_OUTPUT) | accepted for JSBSim's sake; outputs write nothing, so it has no effect |

Writing `simulation/reset` does the same from a script or a property
write, the written integer read as these flags.

A reset returns to the initial conditions, which a [trim](trim.md) does not
change: after a reset the aircraft is untrimmed again. To return to an
exact earlier state, trimmed or not, use a [snapshot](snapshots.md).
