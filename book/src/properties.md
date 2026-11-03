# Properties

The property tree is the simulation's whole interface. Each model publishes
its state under a path, such as `velocities/vc-kts` or
`propulsion/engine[0]/thrust-lbs`, and reads its inputs from others, such as
`fcs/throttle-cmd-norm`. The aircraft XML defines more, and your program
reads and writes the same tree. The paths are JSBSim's, so JSBSim's
documentation and its aircraft files tell you what they are called; a
loaded model's properties are the ones its XML and the models bind.

## By name

[`get_property`](api/jsbsim/struct.Fdm.html#method.get_property) and
[`set_property`](api/jsbsim/struct.Fdm.html#method.set_property) take a
path ([`examples/properties.rs`](https://github.com/luofang34/jsbsim-rs/blob/main/crates/jsbsim/examples/properties.rs)):

```rust
{{#include ../../crates/jsbsim/examples/properties.rs:by_name}}
```

A relative path starts at the executive's root, `/fdm/jsbsim`, as JSBSim's
does; a path beginning with `/` starts at the true root. Values are `f64`;
boolean and integer properties read as 0 and 1, or as whole numbers, and
writes are converted to their type.

## By handle

Parsing a path and walking the tree on every access costs time in a loop
that runs hundreds of times a second. Look a node up once with
[`node`](api/jsbsim/struct.Fdm.html#method.node), which gives a
[`NodeId`](api/jsbsim/struct.NodeId.html), then use
[`read_node`](api/jsbsim/struct.Fdm.html#method.read_node) and
[`write_node`](api/jsbsim/struct.Fdm.html#method.write_node):

```rust
{{#include ../../crates/jsbsim/examples/properties.rs:by_node}}
```

A `NodeId` is small, `Copy` and `Hash`. It is derived from the node's
position and path, so it works on any `Fdm` holding the same path at the
same place: a second executive of the same model, or one a
[snapshot](snapshots.md) was restored onto. Used on an `Fdm` where it names
nothing, it is refused with
[`Error::StaleNode`](api/jsbsim/error/enum.Error.html#variant.StaleNode),
which says what that slot holds instead, and nothing is read or written.
Restoring a snapshot taken before a node was created drops that node, and
its handles become stale.

## What reads and writes do

```rust
{{#include ../../crates/jsbsim/examples/properties.rs:semantics}}
```

- A read calls the model behind a bound property, or evaluates the function
  that produces it, so it is always current. That is why the read methods
  take `&mut self`: evaluating updates cached function and table state.
- A path nothing has created reads `0.0`, as JSBSim's `GetPropertyValue`
  does, and creates nothing. Use `node` to tell an absent property from a
  zero one.
- A write to a path nothing has created **creates it**, as JSBSim does. A
  misspelt path therefore makes a new property silently; look handles up
  with `node`, which creates nothing, when a typo should be an error.
- A write to a read-only property is ignored and returns `Ok(false)`;
  otherwise it returns `Ok(true)`.
- Some writes start an action, such as a trim or a reset; see
  [commands through properties](running.md#commands-through-properties).
- An invalid path, one with characters JSBSim's property tree refuses, is
  [`Error::PropertyPath`](api/jsbsim/error/enum.Error.html#variant.PropertyPath).

## Useful properties

A few of the paths most programs need:

| Path | |
|---|---|
| `fcs/throttle-cmd-norm`, `fcs/mixture-cmd-norm` | engine controls, 0 to 1 (`[n]` for engine *n*) |
| `fcs/elevator-cmd-norm`, `fcs/aileron-cmd-norm`, `fcs/rudder-cmd-norm` | stick and pedals, -1 to 1 |
| `fcs/pitch-trim-cmd-norm`, `fcs/flap-cmd-norm`, `gear/gear-cmd-norm` | trim, flaps, gear |
| `fcs/left-brake-cmd-norm`, `fcs/right-brake-cmd-norm` | brakes |
| `propulsion/magneto_cmd`, `propulsion/starter_cmd` | piston engine start |
| `position/lat-geod-deg`, `position/long-gc-deg`, `position/h-sl-ft`, `position/h-agl-ft` | where |
| `attitude/phi-deg`, `attitude/theta-deg`, `attitude/psi-deg` | attitude |
| `velocities/vc-kts`, `velocities/vtrue-kts`, `velocities/h-dot-fps` | speeds |
| `aero/alpha-deg`, `aero/beta-deg`, `aero/qbar-psf` | air data |
| `accelerations/Nz` | load factor |
| `simulation/sim-time-sec` | time |

Which controls exist, and what they are wired to, is up to each aircraft's
flight control system. For attitude free of the Euler-angle singularity,
see [Position and attitude](pose.md).
