# Introduction

`jsbsim` is an **unofficial** pure-Rust port of
[JSBSim](https://github.com/JSBSim-Team/jsbsim), the open-source flight
dynamics model used by FlightGear, research groups and autopilot test
benches. It is not affiliated with or endorsed by the JSBSim project.

The crate reads the same aircraft, engine, system, initialization and script
XML files as JSBSim, steps the same models in the same order, and is checked
against C++ JSBSim frame by frame (see
[Verification against C++ JSBSim](verification.md)). What changes is how it
fits into a program:

- **No C++ and no build script.** It is plain Rust. It builds for native
  targets and for `wasm32-unknown-unknown`.
- **No I/O of its own.** Model files reach the simulation through a
  [`Resolver`](api/jsbsim/trait.Resolver.html) that you choose: from memory,
  from disk with the `fs` feature, or from wherever your program keeps them.
  `<output>` and `<input>` directives load but open no files or sockets; your
  program reads and writes properties itself.
- **Deterministic.** The same inputs give the same bits on every target,
  native or WebAssembly. A [`Snapshot`](api/jsbsim/snapshot/struct.Snapshot.html)
  of the whole dynamic state can be stored, sent elsewhere and restored, for
  rewind, replay or lockstep.
- **Typed errors.** Loading and stepping return
  [`Result`](api/jsbsim/error/enum.Error.html), with the file and line of the
  XML element at fault. A construct the port does not implement fails the
  load instead of being skipped.
- **Host-owned terrain.** A [`GroundCallback`](api/jsbsim/trait.GroundCallback.html)
  tells the simulation where the ground is, and how it moves.

## How to read this manual

[Getting started](installation.md) adds the crate to a project, drops a box
from 1000 ft and flies JSBSim's Cessna 172. The [user guide](lifecycle.md)
takes each part of the API in the order a program meets it. The
[reference](support.md) says which parts of JSBSim are ported and where the
port deliberately differs. [Development](verification.md) is for working on
the crate itself: how it is verified, built and released.

Every Rust listing in this manual is part of an example in
[`crates/jsbsim/examples`](https://github.com/luofang34/jsbsim-rs/tree/main/crates/jsbsim/examples),
which CI compiles and runs, so the listings cannot drift from the crate. Run
any of them with `cargo run -p jsbsim --features fs --example <name>`.
Links to items such as [`Fdm::run`](api/jsbsim/struct.Fdm.html#method.run)
lead to the API documentation built from the same commit as this manual.

If you know C++ JSBSim, most names carry over: `Fdm` is `FGFDMExec`, a
property path means the same thing, and the API documentation names the C++
function each method mirrors.
