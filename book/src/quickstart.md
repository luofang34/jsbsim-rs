# Quickstart

The smallest aircraft JSBSim loads is a box: a mass, its inertia, a point
that touches the ground, and no aerodynamics. This example keeps it in
memory, so it needs no data files
([`examples/quickstart.rs`](https://github.com/luofang34/jsbsim-rs/blob/main/crates/jsbsim/examples/quickstart.rs)).

```rust
{{#include ../../crates/jsbsim/examples/quickstart.rs:aircraft}}
```

Load it, start it 1000 ft above sea level, and step five seconds:

```rust
{{#include ../../crates/jsbsim/examples/quickstart.rs:run}}
```

```sh
cargo run -p jsbsim --example quickstart
```

It logs the box about 400 ft lower, falling at about 160 ft/s: five
seconds of free fall, as the example's own assertions check.

What happened, step by step:

1. [`MemoryResolver`](api/jsbsim/struct.MemoryResolver.html) maps paths to
   file contents. The simulation reads every file through a
   [`Resolver`](api/jsbsim/trait.Resolver.html) and never opens files
   itself; see [Files and resolvers](resolvers.md).
2. [`Fdm::new`](api/jsbsim/struct.Fdm.html#method.new) makes an
   executive, JSBSim's `FGFDMExec`: the models, the property tree and the
   clock. One `Fdm` holds one aircraft.
3. [`load_model`](api/jsbsim/struct.Fdm.html#method.load_model) reads
   `aircraft/box/box.xml`: the directory, then the model name twice, as
   JSBSim lays out its aircraft.
4. Writing `ic/h-sl-ft` sets an initial condition, and
   [`run_ic`](api/jsbsim/struct.Fdm.html#method.run_ic) applies the initial
   conditions to the models. See
   [Initial conditions and resets](initial-conditions.md).
5. Each [`run`](api/jsbsim/struct.Fdm.html#method.run) advances one time
   step, 1/120 s unless something sets another. See
   [Stepping and the clock](running.md).
6. [`get_property`](api/jsbsim/struct.Fdm.html#method.get_property) reads
   any value in the property tree by its JSBSim path. See
   [Properties](properties.md).

Next, [fly a real aircraft](jsbsim-data.md) from a JSBSim data tree.
