# Scripts

A JSBSim script names an aircraft and an initialization file, sets up
properties, and changes them over time through events. Load one with
[`load_script`](api/jsbsim/struct.Fdm.html#method.load_script), apply its
initial conditions with [`run_ic`](api/jsbsim/struct.Fdm.html#method.run_ic),
and step until [`run`](api/jsbsim/struct.Fdm.html#method.run) returns
`false`:

```rust
{{#include ../../crates/jsbsim/examples/fly_c172.rs:script}}
```

Scripts are written exactly as for JSBSim; the scripts in JSBSim's own
`scripts/` directory run unchanged. In outline:

```xml
<runscript name="C172 takeoff">
  <use aircraft="c172x" initialize="reset00"/>
  <run start="0" end="200" dt="0.0083333">
    <property value="1"> fcs/left-brake-cmd-norm </property>

    <event name="engine start">
      <condition> simulation/sim-time-sec >= 0.25 </condition>
      <set name="fcs/throttle-cmd-norm" value="1.0" action="FG_RAMP" tc="0.5"/>
      <set name="propulsion/starter_cmd" value="1"/>
      <notify>
        <property caption="Speed">velocities/vc-kts</property>
      </notify>
    </event>
  </run>
</runscript>
```

## What a script sets up

- `<use aircraft initialize>` loads the aircraft from the aircraft path and
  its initialization file from the aircraft's directory.
- `<run start end dt>` sets the clock and the time step. `run` returns
  `false` once the time passes `end` (JSBSim adds 0.99 of a time step to it,
  and so does the port).
- `<property value="...">` declares a property and gives it a value. On a
  [reset](initial-conditions.md#resetting) it goes back to that value unless
  it is declared `persistent="true"`.
- The script's own `<input>` and `<output>` load as an aircraft's do,
  writing and reading nothing.

## Events

Each frame, before the models run, every armed event's `<condition>` is
tested. When it holds, the event fires, after its `<delay>` if it has one:

- each `<set>` changes a property: to `value`, or by `value` with
  `type="FG_DELTA"`, or to the result of a `<function>`. With
  `action="FG_STEP"` (the default) the change is immediate; `FG_RAMP`
  moves linearly over `tc` seconds and `FG_EXP` approaches exponentially
  with time constant `tc`;
- `<notify>` logs the event's name, time, description and the properties
  it lists, at `info` level through `tracing`. Its `format="kml"` is not
  read; notifies always log plain lines.

An event fires once, unless it is `persistent="true"`, which re-arms it when
its condition stops holding, or `continuous="true"`, which keeps applying
its sets while the condition holds. A `<set>` that names a property nothing
has created by the time the event fires is an error, returned from `run`.

## Driving a script from your program

The script and your program share the property tree, so both can act on
the aircraft: the script fires its events inside `run`, and your writes
between `run`s take effect in the next frame. A script that writes
`simulation/do_simple_trim` trims the aircraft inside `run`, and a trim that
does not converge comes back as `run`'s error.

[`reset_to_initial_conditions`](api/jsbsim/struct.Fdm.html#method.reset_to_initial_conditions)
starts a script again from its start time, with its events re-armed.
