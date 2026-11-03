# Files and resolvers

The core performs no I/O. Every file a load reads (the aircraft, the files
it includes, engines, systems, initialization files, scripts) is asked of a
[`Resolver`](api/jsbsim/trait.Resolver.html), which maps a path to its
bytes:

- [`resolve`](api/jsbsim/trait.Resolver.html#tymethod.resolve) returns the
  file's contents, or
  [`Error::ResourceNotFound`](api/jsbsim/error/enum.Error.html#variant.ResourceNotFound)
  when nothing is at the path, or
  [`Error::ResourceUnreadable`](api/jsbsim/error/enum.Error.html#variant.ResourceUnreadable)
  when something is there that cannot be read.
- [`exists`](api/jsbsim/trait.Resolver.html#method.exists) says whether a
  file is there. JSBSim probes several places for an include, and the
  default implementation calls `resolve`; override it when a cheaper check
  exists.

Paths are relative to the resolver's root and use `/`. Before asking, the
loader normalizes them: `.` and empty segments go, `name/..` collapses, and
a leading `..` or `/` stays. A resolver therefore sees the same path for
the same file however the XML spelled it.

## The resolvers in the crate

[`MemoryResolver`](api/jsbsim/struct.MemoryResolver.html) holds files in a
map from path to bytes. It suits tests, models built at run time, files
bundled with `include_str!`, and WebAssembly, where your program fetches the
files and inserts them.

[`FileResolver`](api/jsbsim/struct.FileResolver.html), with the `fs`
feature, reads files below a root directory. It is a convenience, **not a
sandbox**: an absolute path, or a leading `..`, reaches outside the root,
as JSBSim's own path handling does. Serve untrusted model trees through
`MemoryResolver` or a resolver of your own that checks the path.

## Where includes are found

The loader asks for the same places C++ JSBSim searches, in the same order,
and takes the first that [`exists`](api/jsbsim/trait.Resolver.html#method.exists):

| Include | Searched |
|---|---|
| `file="..."` in the aircraft, such as a split-out `<aerodynamics file="aero"/>` | the aircraft's directory |
| `<system file="...">` | the aircraft's directory, its `Systems/` and `systems/`, then the systems path |
| `<engine file="...">`, `<thruster file="...">` | the aircraft's `Engines/`, `engines/`, `Engine/` and `engine/`, then the engine path (not the aircraft's directory itself) |
| `<output file="...">` | the script's directory, for a script's outputs, then the aircraft's |

An include without an extension gets `.xml`. The systems path is `systems`
and the engine path is `engine` until
[`set_systems_path`](api/jsbsim/struct.Fdm.html#method.set_systems_path) or
[`set_engine_path`](api/jsbsim/struct.Fdm.html#method.set_engine_path) says
otherwise. An include found nowhere fails the load with
[`XmlError::IncludeNotFound`](api/jsbsim/error/enum.XmlError.html#variant.IncludeNotFound),
naming the including element.

This example serves an aircraft from memory; its `<system file="beacon"/>`
is found in the shared `systems/` directory
([`examples/resolvers.rs`](https://github.com/luofang34/jsbsim-rs/blob/main/crates/jsbsim/examples/resolvers.rs)):

```rust
{{#include ../../crates/jsbsim/examples/resolvers.rs:memory}}
```

## A resolver of your own

Implement the trait to read from an archive, an asset system, a network
cache, or a layering of sources. This one lays local edits over a shared,
read-only collection:

```rust
{{#include ../../crates/jsbsim/examples/resolvers.rs:overlay}}
```

Because the aircraft's own `Systems/` directory comes before the shared
`systems/` directory in the search, a local copy there wins:

```rust
{{#include ../../crates/jsbsim/examples/resolvers.rs:overlay_use}}
```

Keep `resolve` and `exists` consistent: the loader uses `exists` to choose
among candidate paths and `resolve` to read the one it chose.

## Resolvers and snapshots

A [snapshot](snapshots.md) records a fingerprint of the bytes of every
document loaded, never their paths. Two executives that loaded the same
bytes, one from disk and one from memory or over the network, accept each
other's snapshots.
