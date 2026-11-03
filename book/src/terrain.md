# Terrain

By default the ground is level, at sea level (or at the terrain elevation
an initial condition sets), on the planet's ellipsoid, and at rest. A
program with terrain of its own (a height map, a ship's deck, another
simulation's world) supplies it through a
[`GroundCallback`](api/jsbsim/trait.GroundCallback.html), the port of
JSBSim's `FGGroundCallback`.

The callback answers one question,
[`ag_level`](api/jsbsim/trait.GroundCallback.html#tymethod.ag_level): given
a time and a [`Location`](api/jsbsim/math/struct.Location.html), where is the
ground below it? It returns a [`Contact`](api/jsbsim/struct.Contact.html):
the height above the ground, the ground point, the surface normal in
Earth-centred, Earth-fixed (ECEF) axes, and how the surface moves.

This callback is a plateau 500 ft above a spherical Earth
([`examples/terrain.rs`](https://github.com/luofang34/jsbsim-rs/blob/main/crates/jsbsim/examples/terrain.rs)):

```rust
{{#include ../../crates/jsbsim/examples/terrain.rs:plateau}}
```

Install it before loading, so that initial conditions given above ground
are placed on it:

```rust
{{#include ../../crates/jsbsim/examples/terrain.rs:install}}
```

It stays installed across resets, planet loads and snapshot restores.

## Moving surfaces

[`Contact::with_motion`](api/jsbsim/struct.Contact.html#method.with_motion)
gives the surface a velocity and an angular velocity, both in ECEF axes.
The landing gear uses the velocity of the surface at each contact point, so
for a rotating surface return its translation plus the angular velocity
crossed with the point's offset from the centre of rotation. The angular
velocity itself reaches only the friction solution. A box resting on this
deck is carried along with it:

```rust
{{#include ../../crates/jsbsim/examples/terrain.rs:deck}}
```

## The contract

The callback is called several times per frame and per contact point, from
inside the frame. It must:

- not block, do I/O or allocate; answer from data already in memory;
- be deterministic, a function of its arguments and its own state, if
  [snapshots](snapshots.md) and replays are to reproduce runs;
- be `Send`, so that the `Fdm` holding it can move between threads.

Read only the ECEF coordinates of the location it is given, or copy it and
set its ellipse first: gear positions arrive without an ellipse. Set the
planet's ellipse on the ground point you return and call
[`cache_derived`](api/jsbsim/math/struct.Location.html#method.cache_derived)
on it, because the simulation reads its geodetic altitude.

Snapshots do not store the callback, and restoring one does not replace it.
Its state, such as a deck's motion, is the host's to save alongside.

## Elevation and planet shape

[`set_terrain_elevation`](api/jsbsim/trait.GroundCallback.html#method.set_terrain_elevation)
receives the elevation an initialization file's `<elevation>`,
`ic/terrain-elevation-ft` or `position/terrain-elevation-asl-ft` asks for,
and [`set_ellipse`](api/jsbsim/trait.GroundCallback.html#method.set_ellipse)
receives the planet's semi-axes when a `<planet>` loads. Both do nothing
unless you implement them; the default terrain follows both.

`ic/h-agl-ft` and an initialization file's altitude above ground are
measured from the ground point your callback returns, along the local
vertical.

## Planets

An aircraft's `<planet>` element changes the planet: its semi-axes,
rotation rate, gravitational parameter and J2, and with
`<atmosphere model="MSIS"/>` or `<atmosphere model="Mars"/>` its
atmosphere (see [JSBSim support](support.md#atmosphere-winds-and-planets)).
`simulation/gravity-model` chooses spherical (0) or WGS84 with J2 (1)
gravity.
