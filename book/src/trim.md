# Trimming

Trimming finds the control settings and attitude at which the aircraft
flies steadily: for level flight, the angle of attack, throttle and pitch
trim at which it neither climbs nor slows nor pitches. JSBSim's trim
routine, `FGTrim`, is ported as it is.

Set the initial conditions and apply them, then trim
([`examples/trim_and_linearize.rs`](https://github.com/luofang34/jsbsim-rs/blob/main/crates/jsbsim/examples/trim_and_linearize.rs)):

```rust
{{#include ../../crates/jsbsim/examples/trim_and_linearize.rs:trim}}
```

[`do_trim`](api/jsbsim/struct.Fdm.html#method.do_trim) returns a
[`TrimReport`](api/jsbsim/trim/struct.TrimReport.html): the mode, whether it
succeeded, the cycles it took, and for each axis an
[`AxisReport`](api/jsbsim/trim/struct.AxisReport.html) with the control's
final value and the state's residual against its tolerance. Values are in
the simulation's units; an angle-of-attack control is in radians.

Once trimmed, the aircraft holds its state:

```rust
{{#include ../../crates/jsbsim/examples/trim_and_linearize.rs:fly_trimmed}}
```

## Modes

A [`TrimMode`](api/jsbsim/trim/enum.TrimMode.html) chooses the axes, each a
state to bring to zero and the control that does it:

| Mode | Value | Axes (state by control) |
|---|---|---|
| `Longitudinal` | 0 | w-dot by alpha, u-dot by throttle, q-dot by pitch trim |
| `Full` | 1 | longitudinal, then v-dot by roll angle, p-dot by aileron, r-dot by rudder |
| `Ground` | 2 | w-dot by height, q-dot by pitch, p-dot by roll: settles the aircraft on its gear |
| `Pullup` | 3 | a steady pull-up at the load factor `ic/targetNlf` |
| `Custom` | 4 | the axes given to `do_trim_custom` |
| `Turn` | 5 | a coordinated turn at the initial bank angle |
| `None` | 6 | nothing |

The values are JSBSim's, as written to `simulation/do_simple_trim` and
returned by [`TrimMode::from_i32`](api/jsbsim/trim/enum.TrimMode.html#method.from_i32).

## Custom axes

[`do_trim_custom`](api/jsbsim/struct.Fdm.html#method.do_trim_custom) takes
the axes as ([`State`](api/jsbsim/trim/enum.State.html),
[`Control`](api/jsbsim/trim/enum.Control.html)) pairs, solved in the order
given. A state given twice keeps its first control, as JSBSim's `AddState`
does:

```rust
{{#include ../../crates/jsbsim/examples/trim_and_linearize.rs:custom}}
```

## How it works, and how it fails

The trim works on the executive's state with the clock held. Axis by axis,
it brackets the control value at which the state changes sign and then
solves for it by regula falsi, cycling through the axes until every one is
within tolerance. It gives up after a fixed number of cycles, or when the
last axis outside tolerance cannot change sign over its control's range.

A trim that does not converge returns
[`Error::TrimFailed`](api/jsbsim/error/enum.Error.html#variant.TrimFailed).
The controls it moved are put back, and
[`last_trim`](api/jsbsim/struct.Fdm.html#method.last_trim) still holds its
report, so you can see which axis failed. A trim asked for far outside the
aircraft's envelope, such as level flight below its stall speed, will not
converge; adjust the initial conditions and try again.

On success, `simulation/trim-completed` reads 1 and
[`trims_run`](api/jsbsim/struct.Fdm.html#method.trims_run) counts it. The
trimmed state lives in the models and controls, not in the initial
conditions: a [reset](initial-conditions.md#resetting) returns to the
untrimmed state, while a [snapshot](snapshots.md) taken after the trim
returns to the trimmed one. With `simulation/jsbsim-debug` above zero, the
report is also logged in JSBSim's format.

## Trimming from a script or a property

Writing a mode's value to `simulation/do_simple_trim` trims as `do_trim`
does, inside the write. Scripts trim this way:

```xml
<event name="Trim">
  <condition> simulation/sim-time-sec gt 0.1 </condition>
  <set name="simulation/do_simple_trim" value="1"/>
</event>
```

A failure there comes back from [`run`](api/jsbsim/struct.Fdm.html#method.run).
An invalid mode is `TrimFailed` with JSBSim's message, `Illegal trimming
mode!`.

## Trims an initialization file asks for

An initialization file's `<trim>` element records a mode, which
[`trim_requested`](api/jsbsim/struct.Fdm.html#method.trim_requested)
returns. As in JSBSim's library, loading does not run it; JSBSim's
command-line program does that after `run_ic`. Do the same if you want it:
after `run_ic`, if `trim_requested()` is not `TrimMode::None`, call
`do_trim` with it. Following JSBSim, `<trim>1</trim>` asks for a `Ground`
trim and `<trim>0</trim>` for none.
