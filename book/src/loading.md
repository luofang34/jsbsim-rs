# Loading aircraft and scripts

There are three ways to load, each through a
[`Resolver`](api/jsbsim/trait.Resolver.html):

| Method | Reads | The aircraft directory is |
|---|---|---|
| [`load_model(r, "aircraft", "c172x")`](api/jsbsim/struct.Fdm.html#method.load_model) | `aircraft/c172x/c172x.xml` | `aircraft/c172x` |
| [`load_model_file(r, "models/Rascal/Rascal110-JSBSim.xml")`](api/jsbsim/struct.Fdm.html#method.load_model_file) | that file (`.xml` is added if missing) | `models/Rascal` |
| [`load_script(r, "scripts/c1723.xml")`](api/jsbsim/struct.Fdm.html#method.load_script) | the script, then the aircraft and initialization file it names | as `load_model` |

`load_model` follows JSBSim's layout, where each aircraft lives in a
directory of its own name. `load_model_file` loads a model that does not,
such as the JSBSim models PX4 ships. The aircraft directory, which
[`full_aircraft_path`](api/jsbsim/struct.Fdm.html#method.full_aircraft_path)
reports once a load has found the model, is where includes are searched
first (see [Files and resolvers](resolvers.md#where-includes-are-found)).

`load_script` finds the aircraft in the aircraft path, `aircraft` unless
[`set_aircraft_path`](api/jsbsim/struct.Fdm.html#method.set_aircraft_path)
changes it, and the initialization file in that aircraft's directory. It
also sets the time step and start time from the script's `<run>`, declares
the script's properties and arms its events (see [Scripts](scripts.md)).
None of the three applies the initial conditions; call
[`run_ic`](api/jsbsim/struct.Fdm.html#method.run_ic) after loading.

## What a load reads

The aircraft document is read in JSBSim's order: `<planet>`, then the
required `<metrics>`, `<mass_balance>` and `<ground_reactions>`, then
`<external_reactions>`, `<buoyant_forces>`, `<propulsion>`, every
`<system>`, `<autopilot>`, `<flight_control>`, `<aerodynamics>`, and
finally `<input>` and `<output>`. Model-level `<property>` declarations and
`<function>`s load alongside. [JSBSim support](support.md) says what each
section supports.

`<output>` and `<input>` load as in JSBSim, with their properties and
functions, but write and read nothing: the core opens no files or sockets.
Each `<output>` logs a line saying so. Log what you need from the property
tree yourself.

## One load per executive

Each `Fdm` holds one aircraft. Once a load has found its model document,
another `load_model`, `load_model_file` or `load_script` on the same `Fdm`
is refused with [`Error::Model`](api/jsbsim/error/enum.Error.html#variant.Model);
JSBSim would instead throw the first aircraft away. A model document that is
not found begins no load, so another attempt may follow. A load that fails
part way is not rolled back: drop that `Fdm` and start again with a new one.

## Strict loading

C++ JSBSim logs some mistakes in a model and carries on: an operation name
it does not know, an unknown flight-control component, a gain without a
`<gain>`, a contact with an unknown table, and the like. By default the
port does the same, logging each at `error` level through `tracing`, so
that every model JSBSim accepts loads here too.

[`LoadOptions`](api/jsbsim/struct.LoadOptions.html) with
[`with_strict(true)`](api/jsbsim/struct.LoadOptions.html#method.with_strict)
turns those into errors instead, which is what you want while writing a
model:

```rust
{{#include ../../crates/jsbsim/examples/errors.rs:strict}}
```

Strict mode covers functions and tables, flight-control components, sensors
and ground contacts, where JSBSim's lenient paths are. It does not change
how a model behaves once loaded, and it is not needed to be told about
unported features: a construct the port does not implement fails the load
with [`Error::Unsupported`](api/jsbsim/error/enum.Error.html#variant.Unsupported)
whatever the options (see [JSBSim support](support.md#what-is-not-ported)).

## Paths of shared directories

[`set_systems_path`](api/jsbsim/struct.Fdm.html#method.set_systems_path)
and [`set_engine_path`](api/jsbsim/struct.Fdm.html#method.set_engine_path)
name the directories searched for systems and engines after the aircraft's
own, relative to the resolver's root. Set them before loading.

## Wall-clock date

JSBSim's `<magnetometer>` dates its magnetic field model by the time of day
the aircraft loads. The core reads no clock: give it the date with
[`set_wall_clock`](api/jsbsim/struct.Fdm.html#method.set_wall_clock) (Unix
seconds) before loading, or it uses 1 January 1970.
