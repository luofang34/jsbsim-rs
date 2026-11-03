# Snapshots

A [`Snapshot`](api/jsbsim/snapshot/struct.Snapshot.html) holds the whole
dynamic state of an executive: every property's value, every model's
state, the random generators, the script's event bookkeeping, the clock
and the frame counter. Restoring it puts the executive back exactly, to the
bit. JSBSim has no equivalent; it is there for rewind, replay, lockstep
networking and checkpoints.

## Rewind

Take a snapshot with
[`snapshot_into`](api/jsbsim/struct.Fdm.html#method.snapshot_into) and go
back to it with [`restore`](api/jsbsim/struct.Fdm.html#method.restore)
([`examples/snapshots.rs`](https://github.com/luofang34/jsbsim-rs/blob/main/crates/jsbsim/examples/snapshots.rs)):

```rust
{{#include ../../crates/jsbsim/examples/snapshots.rs:rewind}}
```

`snapshot_into` writes into a `Snapshot` you keep, reusing its buffer:
after the first, neither taking nor restoring allocates. Keep a ring of
snapshots for a rewind buffer.

## Replay

The simulation is deterministic, so the same inputs from the same state
give the same results:

```rust
{{#include ../../crates/jsbsim/examples/snapshots.rs:replay}}
```

A replay system can therefore store occasional snapshots plus the inputs
between them, and reproduce any frame exactly.

## Moving state between executives

[`as_bytes`](api/jsbsim/snapshot/struct.Snapshot.html#method.as_bytes)
gives the snapshot as bytes, and
[`Snapshot::from_bytes`](api/jsbsim/snapshot/struct.Snapshot.html#method.from_bytes)
reads them back. The bytes are the same on every target, so a snapshot
taken natively restores in WebAssembly and the other way round:

```rust
{{#include ../../crates/jsbsim/examples/snapshots.rs:transfer}}
```

## What must match

A snapshot holds state, not structure, so it restores only onto an
executive that loaded the same model the same way. Its
[`Fingerprint`](api/jsbsim/snapshot/struct.Fingerprint.html) records:

- the bytes of every document loaded, in order (never their paths, so it
  does not matter where the files came from);
- the property tree as loading left it;
- the model's structure: how many components, engines, tanks, contacts and
  functions, and of which kinds.

Restoring onto an executive whose fingerprint differs fails with
[`SnapshotError::FingerprintMismatch`](api/jsbsim/snapshot/enum.SnapshotError.html#variant.FingerprintMismatch),
naming the first part that differs, and leaves the executive unchanged:

```rust
{{#include ../../crates/jsbsim/examples/snapshots.rs:mismatch}}
```

[`Fdm::fingerprint`](api/jsbsim/struct.Fdm.html#method.fingerprint) and
[`Snapshot::fingerprint`](api/jsbsim/snapshot/struct.Snapshot.html#method.fingerprint)
let you check before restoring, for example when a peer joins a session,
and [`first_difference`](api/jsbsim/snapshot/struct.Fingerprint.html#method.first_difference)
says which part differs.

## Guarantees

- **Atomic.** `restore` decodes and checks the whole snapshot before it
  writes anything. A corrupt, truncated or mismatched snapshot returns an
  error and leaves the executive as it was.
- **Properties created at run time** (by a script, or by your writes) are
  carried by name. Restoring re-creates those the snapshot holds and drops
  those created after it; [`NodeId`](api/jsbsim/struct.NodeId.html)s of
  dropped nodes become stale.
- **Between frames only.** Take snapshots between `run`s. A trim, reset or
  linearization started by a property write has finished when the write
  returns, so there is never one half done.
- **Not saved:** the [ground callback](terrain.md), which belongs to your
  program and stays installed; and the results of the last trim and
  linearization, which are reports rather than state.

## The format

One little-endian byte stream: an 8-byte magic
([`MAGIC`](api/jsbsim/snapshot/constant.MAGIC.html)), the
[`FORMAT_VERSION`](api/jsbsim/snapshot/constant.FORMAT_VERSION.html), the
fingerprint, then the body. Floating-point values are stored as their
bits. A stream from another format version is refused with
[`SnapshotError::VersionMismatch`](api/jsbsim/snapshot/enum.SnapshotError.html#variant.VersionMismatch);
store the crate version alongside snapshots you keep for long.
