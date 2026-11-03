# Flying a JSBSim aircraft

JSBSim keeps its models in a *data tree*: a directory with `aircraft/`,
`engine/`, `systems/` and `scripts/` in it. A checkout of JSBSim is one; this
repository keeps a copy of the files its tests use in
[`oracle/inputs`](https://github.com/luofang34/jsbsim-rs/tree/main/oracle/inputs).
With the `fs` feature, [`FileResolver`](api/jsbsim/struct.FileResolver.html)
serves such a tree from disk.

This example runs JSBSim's own `scripts/c1723.xml`, in which a Cessna 172
starts its engine, takes off and climbs on autopilot
([`examples/fly_c172.rs`](https://github.com/luofang34/jsbsim-rs/blob/main/crates/jsbsim/examples/fly_c172.rs)).
It finds the tree here:

```rust
{{#include ../../crates/jsbsim/examples/fly_c172.rs:root}}
```

and flies the script to its end:

```rust
{{#include ../../crates/jsbsim/examples/fly_c172.rs:script}}
```

```sh
cargo run -p jsbsim --features fs --release --example fly_c172
# or against your own JSBSim checkout:
JSBSIM_ROOT=~/jsbsim cargo run -p jsbsim --features fs --release --example fly_c172
```

The log shows what JSBSim would print: the script's `<notify>` events, the
takeoff report of each gear, and notes about what the port leaves to your
program, such as `<output> writes nothing`, because the core opens no files.

A script is one way in. [`load_script`](api/jsbsim/struct.Fdm.html#method.load_script)
loads the aircraft and the initialization file the script names, sets the
time step, declares the script's properties and arms its events; each
[`run`](api/jsbsim/struct.Fdm.html#method.run) then fires the events whose
conditions hold, and returns `false` once the script's end time has passed.
Your program can instead load an aircraft and drive it itself:

- [Loading aircraft and scripts](loading.md) loads a model without a script.
- [Initial conditions and resets](initial-conditions.md) places it.
- [Properties](properties.md) moves its controls and reads its state.
- [Trimming](trim.md) finds the control settings for steady flight.

[`NodeId`](api/jsbsim/struct.NodeId.html), used above for the altitude, is a
handle to one property: look the path up once with
[`node`](api/jsbsim/struct.Fdm.html#method.node), then read or write it
every frame without parsing the path again.
