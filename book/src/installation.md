# Installation

Add the crate to your project:

```sh
cargo add jsbsim
# or, to read model files from disk with FileResolver:
cargo add jsbsim --features fs
```

which puts this in `Cargo.toml`:

```toml
[dependencies]
jsbsim = { version = "0.2", features = ["fs"] }
```

The minimum supported Rust version is the `rust-version` in the
[workspace manifest](https://github.com/luofang34/jsbsim-rs/blob/main/Cargo.toml).
The crate has no build script and no C or C++ dependency; it depends on a
handful of pure-Rust crates (`libm`, `roxmltree`, `sha2`, `thiserror`,
`tracing`), listed in its
[manifest](https://github.com/luofang34/jsbsim-rs/blob/main/crates/jsbsim/Cargo.toml).

## Features

| Feature | What it adds |
|---|---|
| `fs` | [`FileResolver`](api/jsbsim/struct.FileResolver.html), which reads model files from a directory on disk. |

Without `fs` the crate never touches the filesystem; serve files with
[`MemoryResolver`](api/jsbsim/struct.MemoryResolver.html) or your own
[`Resolver`](api/jsbsim/trait.Resolver.html). See
[Cargo features](features.md) for the internal features you should leave
off.

## WebAssembly

The crate builds for `wasm32-unknown-unknown` as it is:

```sh
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown
```

It needs no JavaScript glue, reads no clock and spawns no threads. In a
browser, fetch the aircraft files yourself and hand them to a
`MemoryResolver`; see [Threads, determinism and WebAssembly](determinism.md).

## Model data

JSBSim's aircraft, engines, systems and scripts are not part of the crate.
Use a checkout of [JSBSim](https://github.com/JSBSim-Team/jsbsim) (its
`aircraft/`, `engine/`, `systems/` and `scripts/` directories), your own
models, or the copy of JSBSim's files this repository keeps for its tests in
[`oracle/inputs`](https://github.com/luofang34/jsbsim-rs/tree/main/oracle/inputs).
The examples in this manual read that copy unless `JSBSIM_ROOT` names
another tree.

## Logging

The crate reports what it skips, what a script's `<notify>` prints and
similar news through [`tracing`](https://docs.rs/tracing). Install a
subscriber to see them, for example with `tracing-subscriber`:

```sh
cargo add tracing-subscriber
```

and `tracing_subscriber::fmt().init();` at the start of `main`. Without a
subscriber the events cost next to nothing and go nowhere. See
[Errors and logging](errors.md).
