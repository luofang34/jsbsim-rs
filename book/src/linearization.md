# Linearization

[`do_linearization`](api/jsbsim/struct.Fdm.html#method.do_linearization)
computes the state-space model of the aircraft about its current state,
usually a [trimmed](trim.md) one:

```text
dx/dt = A x + B u
    y = C x + D u
```

It perturbs each state and input in turn, through the simulation itself,
and takes central differences, as JSBSim's `FGLinearization` does. JSBSim
writes the result to a Scicoslab file; the port writes nothing and returns
a [`Linearization`](api/jsbsim/linearization/struct.Linearization.html)
instead
([`examples/trim_and_linearize.rs`](https://github.com/luofang34/jsbsim-rs/blob/main/crates/jsbsim/examples/trim_and_linearize.rs)):

```rust
{{#include ../../crates/jsbsim/examples/trim_and_linearize.rs:linearize}}
```

## What it holds

| Field | |
|---|---|
| `x`, `u`, `y` | the states, inputs and outputs, as [`Component`](api/jsbsim/linearization/enum.Component.html)s |
| `x0`, `u0`, `y0` | their values at the operating point |
| `a`, `b`, `c`, `d` | the matrices, as [`Matrix`](api/jsbsim/linearization/struct.Matrix.html)es: rows are derivatives or outputs, columns are states or inputs |

[`x_names`](api/jsbsim/linearization/struct.Linearization.html#method.x_names)
and [`x_units`](api/jsbsim/linearization/struct.Linearization.html#method.x_units)
(and their `u_` and `y_` twins) give the names and units JSBSim prints.
`Matrix` is dense and row-major:
[`get(row, col)`](api/jsbsim/linearization/struct.Matrix.html#method.get)
returns `None` outside it,
[`rows`](api/jsbsim/linearization/struct.Matrix.html#method.rows) iterates
the rows, and
[`as_slice`](api/jsbsim/linearization/struct.Matrix.html#method.as_slice)
hands all of it to a linear-algebra crate.

The components are JSBSim's:

- **States**: true airspeed, angle of attack, pitch angle and pitch rate;
  then, when the first engine drives a propeller, the speed of each
  propeller (up to four); then sideslip, roll angle and rate, heading and
  yaw rate, latitude, longitude and altitude.
- **Inputs**: throttle (all engines together), aileron, elevator and rudder
  commands.
- **Outputs**: the states themselves (full state feedback), so `C` is
  the identity and `D` zero, to within the differencing.

## Conditions and side effects

The aircraft needs an engine; without one, `do_linearization` returns
[`Error::Model`](api/jsbsim/error/enum.Error.html#variant.Model). Each
perturbation reinitializes the aircraft from the initial conditions with
every engine running, so afterwards the initial conditions hold the
operating point and the aircraft is left reinitialized there. The time step
and simulation time are restored.

[`last_linearization`](api/jsbsim/struct.Fdm.html#method.last_linearization)
keeps the latest result. Writing `simulation/do_linearization` linearizes
too, so a script can ask for it; read the result from
`last_linearization`.
