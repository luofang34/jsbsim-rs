# Stepping and the clock

[`run`](api/jsbsim/struct.Fdm.html#method.run) advances the simulation one
frame (see [How a simulation runs](lifecycle.md#one-frame)) and returns
whether it should go on:

- `Ok(true)`: carry on.
- `Ok(false)`: the loaded script's end time has passed, or something wrote
  a non-zero value to `simulation/terminate`. Stepping further is allowed;
  `false` is advice, as in JSBSim.
- `Err(..)`: the frame failed, for example a function that could not be
  evaluated, a script `<set>` naming a property that does not exist, a
  [trim](trim.md) a script asked for that did not converge, or stepping an
  `Fdm` with no aircraft loaded.

A typical loop runs until the script ends, or until your own condition:

```rust
{{#include ../../crates/jsbsim/examples/fly_c172.rs:script}}
```

## Time

| | |
|---|---|
| [`sim_time`](api/jsbsim/struct.Fdm.html#method.sim_time) | simulation time in seconds; also `simulation/sim-time-sec` |
| [`dt`](api/jsbsim/struct.Fdm.html#method.dt) | the time step in seconds; also `simulation/dt` |
| [`set_dt`](api/jsbsim/struct.Fdm.html#method.set_dt) | sets the time step; a script's `<run dt="...">` sets it too |
| [`set_sim_time`](api/jsbsim/struct.Fdm.html#method.set_sim_time) | sets the clock without moving any state |

The time step starts at 1/120 s. The clock adds `dt` every frame, in
JSBSim's order of operations, so the times of two runs agree to the bit.
JSBSim's integrators were tuned at 120 Hz; aircraft with stiff gear or
fast controls may need a smaller step, and few models are happy with a
larger one.

A time step of zero suspends integration: frames still run the models, but
nothing moves, time stands still and script events are not tested.
[`run_ic`](initial-conditions.md#applying-them), [trimming](trim.md) and
[linearization](linearization.md) use this themselves and put the time
step back afterwards.

## Pausing

Writing 1 to `simulation/pause` holds the simulation: time does not advance
and the models do not run, but `run` still returns, and script events are
still tested. Write 0 to go on:

```rust
{{#include ../../crates/jsbsim/examples/properties.rs:hold}}
```

## Frame rate and real time

`run` takes as long as the frame takes to compute and no longer; the core
never sleeps or reads a clock. To fly in real time, call it at the rate of
`dt` from your own loop, or run several frames per display frame. For
batch work, call it as fast as it goes.

## Commands through properties

Some properties start an action when written, as in JSBSim. The action
completes inside the write, and its errors come back from
[`set_property`](api/jsbsim/struct.Fdm.html#method.set_property) or
[`write_node`](api/jsbsim/struct.Fdm.html#method.write_node), or from `run`
when a script event writes them:

| Property | Action |
|---|---|
| `simulation/do_simple_trim` | trim in the [mode](trim.md#modes) written |
| `simulation/do_linearization` | [linearize](linearization.md) |
| `simulation/reset` | [reset](initial-conditions.md#resetting), the value read as a reset mask |
| `simulation/terminate` | non-zero makes `run` return `false` |
| `simulation/pause` | non-zero holds the simulation |
| `simulation/randomseed` | reseeds the random generator |
| `propulsion/set-running` | starts engine *n*, or every engine for -1 |
| `simulation/models/<model>/enabled` | 0 stops that model running, as in JSBSim |
