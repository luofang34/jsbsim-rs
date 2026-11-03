# JSBSim support

The port aims to load every model C++ JSBSim loads and to behave as JSBSim
does with it. This chapter says what that covers, and how to find out about
a particular model.

## How to tell whether a model is supported

Load it. Three outcomes are possible:

- **It loads.** The port implements everything the model uses. Anything
  JSBSim itself would have logged and ignored is logged at `error` level;
  load with [strict mode](loading.md#strict-loading) to make those errors
  too.
- **[`Error::Unsupported`](api/jsbsim/error/enum.Error.html#variant.Unsupported).**
  The model uses a construct the port does not implement; the error names
  the construct and its file and line. The port refuses rather than
  silently simulating something different.
- **Another error.** JSBSim would refuse the model too, or would misbehave
  on it; the error says what is wrong.

The oracle's model check loads every aircraft in JSBSim's tree in both C++
JSBSim and the port and compares the properties they bind. Its expected
outcome per aircraft is committed in
[`oracle/model-loading.txt`](https://github.com/luofang34/jsbsim-rs/blob/main/oracle/model-loading.txt),
where a model the port could not load would be listed with its
`Unsupported` construct.

## What is ported

**Aircraft sections**: `<planet>`, `<metrics>`, `<mass_balance>` with point
masses, `<ground_reactions>` (gear and structural contacts, brakes,
steering, retraction), `<external_reactions>`, `<buoyant_forces>` (gas
cells and ballonets), `<propulsion>`, `<system>`, `<autopilot>`,
`<flight_control>`, `<aerodynamics>` in all of JSBSim's axis systems,
model-level `<property>` declarations and `<function>`s, and
`<input>`/`<output>`, which load without doing I/O.

**Flight control components**: every component type JSBSim's `FGFCS`
knows: filters (lag, lead-lag, washout, second order, integrator), gains
(pure, scheduled, aerosurface scale), summer, deadband, switch, kinematic,
function, PID, actuator, linear actuator, distributor, angle, sensors
(sensor, accelerometer, gyro, magnetometer) and waypoint heading and
distance.

**Functions**: every operation of JSBSim's `FGFunction`, function templates,
`<random>` and `<urandom>`, tables of one, two and more dimensions, internal
tables, and conditions.

**Propulsion**: piston, turbine, turboprop, rocket, electric and brushless
DC engines; propeller, rotor, nozzle and direct thrusters; tanks, including
solid-propellant grains (cylindrical and end-burning).

**Initialization files**: formats 1 and 2. **Scripts**: `<use>`, `<run>`,
property declarations, events with conditions, delays, sets (step, ramp,
exponential; value, delta or function) and notifies.

**Executive**: trimming in every mode, linearization, resets, pause and
the `simulation/` commands.

### Atmosphere, winds and planets

The 1976 U.S. Standard Atmosphere, with humidity; NRLMSISE-00 and the Mars
atmosphere, selected by `<planet><atmosphere model="MSIS"/>` or
`model="Mars"`; steady wind, gusts, the one-minus-cosine gust and
turbulence (Culp, and the Dryden models of MIL-STD-1797A and Tustin); and
planets of any ellipsoid, rotation and gravity, with spherical or J2
gravity.

## What is not ported

Each construct the port does not implement is one call to `unsupported(`
in the crate's source, which turns it into `Error::Unsupported`; a
[code search](https://github.com/search?q=repo%3Aluofang34%2Fjsbsim-rs+path%3Acrates%2Fjsbsim%2Fsrc+%22unsupported%28%22&type=code)
lists them. One follows from the design rather than from
porting effort: `<random>` and `<urandom>` with `seed="time_now"` need a
clock, which the core does not read.

What the port does instead of I/O, and the few places it deliberately
differs from C++ JSBSim, are in
[Differences from C++ JSBSim](differences.md).
