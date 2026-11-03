# Threads, determinism and WebAssembly

## Threads

[`Fdm`](api/jsbsim/struct.Fdm.html) is `Send` but not `Sync`. It can move
to another thread, and only one thread may use it at a time. Functions and
tables keep small caches in `Cell`s, and a
[ground callback](terrain.md) need only be `Send`. To share one executive
between threads, put it in a `Mutex<Fdm>`.

The core starts no threads. To use several cores, run several executives:
a batch of simulations, one per thread, or one aircraft per thread in a
multi-aircraft program. They share nothing.

## Determinism

Given the same model files, the same initial conditions and the same
inputs, the simulation produces the same bits, on every run and on every
target:

- The models compute in C++ JSBSim's order of operations, which is what
  lets the port match JSBSim.
- No operation that rounds differently between targets is used: no fused
  multiply-add, no platform maths library. `exp` and `pow` are bundled,
  ported from ARM's optimized-routines; the other elementary functions come
  from the pure-Rust [`libm`](https://docs.rs/libm) crate.
- Random numbers (sensor noise, turbulence, `<random>` functions) come from
  generators whose seeds are part of the model or set through
  `simulation/randomseed`, and whose state is in every
  [snapshot](snapshots.md). The core reads no clock; `seed="time_now"` is
  refused with [`Error::Unsupported`](api/jsbsim/error/enum.Error.html#variant.Unsupported).
- Nothing depends on the order of a hash map, the address of an allocation
  or the time a frame takes.

CI runs the same cases natively and as `wasm32-wasip1` under Node, and
fails on the first bit that differs, snapshots included (see
[Verification](verification.md#native-and-webassembly-agree)).

What determinism needs from you: feed inputs at the same frames, keep your
[ground callback](terrain.md) a function of its arguments and state, and
restore its state with each snapshot.

## WebAssembly

The crate builds for `wasm32-unknown-unknown` with no extra setup. In a
browser:

1. Fetch the aircraft's files, and anything they include, with your
   framework's HTTP client, and insert them into a
   [`MemoryResolver`](api/jsbsim/struct.MemoryResolver.html) under their
   tree paths, or write a [`Resolver`](api/jsbsim/trait.Resolver.html) over
   an asset store you already have.
2. Load and run as anywhere else. `run` is synchronous and takes as long as
   the frame's computation; call it from your animation or game loop.
3. Route `tracing` events to the console with a subscriber such as
   `tracing-wasm`, if you want them.

A snapshot from a native server restores in the browser, and the other way
round, so client and server can run the same simulation in lockstep and
compare or exchange state.
