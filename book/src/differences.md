# Differences from C++ JSBSim

The port reproduces JSBSim's behaviour, including its quirks, because
matching it is how the port is verified. Where it differs, it is for one of
the reasons below.

## No I/O

The core opens no files, sockets or clocks:

- **`<output>`** directives load with their properties
  (`simulation/output[n]/...`) and functions, and each logs that it writes
  nothing. Read the properties you want after each `run` and write them
  where you like.
- **`<input>`** directives load with their declarations and functions,
  which run as in JSBSim, but no socket is opened, so nothing arrives
  through them. Write properties yourself.
- **Linearization** returns the matrices instead of writing a Scicoslab
  file. `simulation/write-state-file` and `simulation/force-output` accept
  writes and do nothing.
- **Model files** come through a [`Resolver`](api/jsbsim/trait.Resolver.html).
- **The wall-clock date** a magnetometer needs comes from
  [`set_wall_clock`](api/jsbsim/struct.Fdm.html#method.set_wall_clock), and
  clock-seeded random generators are refused.

## Errors instead of undefined behaviour

Where JSBSim would crash, hang or read freed memory, the port returns an
error: a `<mod>` whose divisor truncates to zero, a property whose function
reads itself, a second aircraft loaded into one executive (JSBSim would
discard the first), an unknown `<input>` type. Conditions JSBSim reports by
throwing an exception come back as `Err`; a trim that does not converge is
[`Error::TrimFailed`](api/jsbsim/error/enum.Error.html#variant.TrimFailed),
JSBSim's `TrimFailureException`.

## What the library leaves to the program

JSBSim's command-line program does some things its library does not, and
the port is the library:

- A trim an initialization file asks for with `<trim>` is recorded, not
  run; see [Trimming](trim.md#trims-an-initialization-file-asks-for).
- Real-time pacing, output to the console, and option parsing are the
  program's.

## Additions

- [Snapshots](snapshots.md), which JSBSim has no equivalent of.
- [`NodeId`](api/jsbsim/struct.NodeId.html) handles for fast property
  access.
- Quaternion and location accessors that avoid Euler-angle singularities;
  see [Position and attitude](pose.md).
- [Strict loading](loading.md#strict-loading).
- Ground callbacks that report the surface's motion, for moving decks; see
  [Terrain](terrain.md).

## Quirks kept on purpose

A few of JSBSim's behaviours surprise newcomers. The port keeps them, so
that models behave as their authors tested them:

- In a version 1 initialization file, `<altitude>` is height above the
  terrain.
- A script runs until 0.99 of a time step past its end time.
- `<trim>1</trim>` in an initialization file asks for a ground trim.
- Reading a property that does not exist gives 0; writing one creates it.
- A trim works on its own copy of the initial conditions, so a reset after
  a trim returns to the untrimmed state.

The API documentation notes such details where they apply, naming the C++
function a method mirrors.
