# How a simulation runs

An [`Fdm`](api/jsbsim/struct.Fdm.html) is JSBSim's executive, `FGFDMExec`.
It owns the models (atmosphere, winds, flight controls, mass balance,
propulsion, aerodynamics, ground reactions, the equations of motion and the
rest), the property tree that connects them, and the clock. A program uses
one in this order:

1. **Make it** with [`Fdm::new`](api/jsbsim/struct.Fdm.html#method.new), or
   [`Fdm::with_options`](api/jsbsim/struct.Fdm.html#method.with_options) to
   load in [strict mode](loading.md#strict-loading).
2. **Install terrain**, if the default (level ground at sea level, at rest)
   will not do, with
   [`set_ground_callback`](api/jsbsim/struct.Fdm.html#method.set_ground_callback),
   before loading. See [Terrain](terrain.md).
3. **Load** one aircraft with
   [`load_model`](api/jsbsim/struct.Fdm.html#method.load_model) or
   [`load_model_file`](api/jsbsim/struct.Fdm.html#method.load_model_file),
   or a script with
   [`load_script`](api/jsbsim/struct.Fdm.html#method.load_script), through
   a [`Resolver`](api/jsbsim/trait.Resolver.html). See
   [Loading aircraft and scripts](loading.md).
4. **Set initial conditions** through `ic/...` properties or an
   initialization file, and apply them with
   [`run_ic`](api/jsbsim/struct.Fdm.html#method.run_ic). See
   [Initial conditions and resets](initial-conditions.md).
5. **Trim**, if wanted, with
   [`do_trim`](api/jsbsim/struct.Fdm.html#method.do_trim), and
   [linearize](linearization.md) about the trimmed state.
6. **Step** with [`run`](api/jsbsim/struct.Fdm.html#method.run), writing
   controls and reading state through [properties](properties.md) between
   steps.
7. **Save and restore** the dynamic state with
   [snapshots](snapshots.md) whenever you need to go back.

## One frame

Each `run` is one frame of JSBSim's loop:

1. The clock advances by the time step, unless the simulation is paused or
   the time step is zero.
2. If a script is loaded, its events are tested and fired.
3. Every model runs in JSBSim's order, each first reading its inputs from
   the models before it.

The order matters to anyone comparing with C++ JSBSim: a control written
before `run` is seen by the flight controls in that frame, and a script
event's `<set>` lands where JSBSim's would. The executive steps exactly as
JSBSim does, so results match it frame for frame (see
[Verification](verification.md)).

## The property tree

Every model publishes its inputs and outputs as properties, named by paths
such as `fcs/elevator-cmd-norm` or `velocities/vc-kts`. The aircraft's XML
reads and writes the same paths: a flight-control channel reads
`fcs/elevator-cmd-norm` and writes `fcs/elevator-pos-rad`, which the
aerodynamics reads. Your program uses the same tree; it is the whole
interface between the simulation and the outside world. See
[Properties](properties.md).

## One aircraft per executive

An `Fdm` loads one aircraft. A second load is refused with
[`Error::Model`](api/jsbsim/error/enum.Error.html#variant.Model); make a new
`Fdm` for another aircraft, or for another copy of the same one. Executives
share nothing, so any number can run side by side, each on its own thread
if you like (see [Threads, determinism and WebAssembly](determinism.md)).
