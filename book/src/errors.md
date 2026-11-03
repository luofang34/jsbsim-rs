# Errors and logging

Every fallible method returns
[`Result<_, jsbsim::Error>`](api/jsbsim/error/enum.Error.html). The core is
written not to panic (its lints forbid `unwrap`, `expect` and `panic!`),
never exits the process and never prints; what
JSBSim would print to the console goes to [`tracing`](https://docs.rs/tracing).

## The error type

| Variant | When |
|---|---|
| [`ResourceNotFound`](api/jsbsim/error/enum.Error.html#variant.ResourceNotFound) | the resolver has no file at a path |
| [`ResourceUnreadable`](api/jsbsim/error/enum.Error.html#variant.ResourceUnreadable) | a file is there but cannot be read |
| [`Xml`](api/jsbsim/error/enum.Error.html#variant.Xml) | malformed XML, a missing element or attribute, a bad number or unit, an include not found ([`XmlError`](api/jsbsim/error/enum.XmlError.html)) |
| [`Function`](api/jsbsim/error/enum.Error.html#variant.Function) | a function, table or condition is malformed or cannot be evaluated ([`FunctionError`](api/jsbsim/error/enum.FunctionError.html)) |
| [`Unsupported`](api/jsbsim/error/enum.Error.html#variant.Unsupported) | a construct the port does not implement, or one JSBSim does not know, under [strict loading](loading.md#strict-loading) |
| [`UnknownInputType`](api/jsbsim/error/enum.Error.html#variant.UnknownInputType) | an `<input>` of a type JSBSim has no class for |
| [`Model`](api/jsbsim/error/enum.Error.html#variant.Model) | a configuration or state JSBSim refuses: a required section missing, an inconsistent initialization file, a second aircraft for one `Fdm` |
| [`TrimFailed`](api/jsbsim/error/enum.Error.html#variant.TrimFailed) | a [trim](trim.md) did not converge, or its mode is invalid |
| [`Geographic`](api/jsbsim/error/enum.Error.html#variant.Geographic) | a planet whose ellipsoid cannot exist |
| [`Snapshot`](api/jsbsim/error/enum.Error.html#variant.Snapshot) | a [snapshot](snapshots.md) does not decode or does not fit ([`SnapshotError`](api/jsbsim/snapshot/enum.SnapshotError.html)) |
| [`PropertyPath`](api/jsbsim/error/enum.Error.html#variant.PropertyPath) | an invalid property path |
| [`StaleNode`](api/jsbsim/error/enum.Error.html#variant.StaleNode) | a [`NodeId`](api/jsbsim/struct.NodeId.html) that names nothing in this `Fdm` |

The enums are `#[non_exhaustive]`, so match with a wildcard arm. They are
`Clone` and `PartialEq`, which makes them easy to assert on in tests
([`examples/errors.rs`](https://github.com/luofang34/jsbsim-rs/blob/main/crates/jsbsim/examples/errors.rs)):

```rust
{{#include ../../crates/jsbsim/examples/errors.rs:kinds}}
```

## Where in the XML

Errors about a file carry a
[`SourceLocation`](api/jsbsim/error/struct.SourceLocation.html): the file's
path as the resolver saw it and the line of the element at fault.
[`Error::location`](api/jsbsim/error/enum.Error.html#method.location)
returns it, and the error's message names it too, as JSBSim's console
output would (`In file aircraft/box/box.xml: line 12`, or
`aircraft/box/box.xml:11:77` for malformed XML):

```rust
{{#include ../../crates/jsbsim/examples/errors.rs:report}}
```

## After an error

A failed load leaves the `Fdm` part loaded; make a new one. A failed
`run` leaves the frame part done, as an exception would in JSBSim. A failed
trim restores the controls it moved. A failed `restore` changes nothing.

## Logging

What JSBSim prints, the port logs through `tracing`, with the module path
as the target:

| Level | What |
|---|---|
| `error` | what JSBSim reports and then ignores: an unknown component, an inconsistency, a file the core will not write. These are the cases [strict loading](loading.md#strict-loading) refuses. |
| `warn` | JSBSim's warnings, such as a property declared twice |
| `info` | a script's `<notify>`, gear takeoff and touchdown reports, notes that an `<output>` writes nothing |
| `debug` | load details, trim progress |

Install any `tracing` subscriber to see them; the examples use
`tracing-subscriber`'s formatter. Filter by target to quiet a noisy model,
for example `RUST_LOG=info,jsbsim::models::ground_reactions=warn` with an
`EnvFilter`.
