# jsbsim

An **unofficial** pure-Rust port of [JSBSim](https://github.com/JSBSim-Team/jsbsim),
the open-source flight dynamics model. It is not affiliated with or endorsed by
the JSBSim project.

The crate has no C++ dependency and performs no I/O of its own: aircraft,
engine, system and script files reach it through a `Resolver`. It is
deterministic and builds for native targets and for `wasm32-unknown-unknown`.
The `fs` feature adds `FileResolver`, which reads them from a directory.

## Loading an aircraft and running

```rust
use jsbsim::{Fdm, MemoryResolver};

let mut files = MemoryResolver::new();
files.insert(
    "aircraft/box/box.xml",
    r#"<fdm_config name="box" version="2.0">
      <metrics> <wingarea unit="FT2"> 1 </wingarea> <wingspan unit="FT"> 1 </wingspan>
        <chord unit="FT"> 1 </chord> </metrics>
      <mass_balance> <ixx> 10 </ixx> <iyy> 10 </iyy> <izz> 10 </izz>
        <emptywt> 100 </emptywt> <location name="CG"> <x>0</x> <y>0</y> <z>0</z> </location>
      </mass_balance>
      <ground_reactions> <contact type="STRUCTURE" name="BOTTOM">
        <location> <x>0</x> <y>0</y> <z>-10</z> </location>
        <spring_coeff> 1000 </spring_coeff> <damping_coeff> 100 </damping_coeff>
      </contact> </ground_reactions>
      <aerodynamics> <axis name="DRAG"/> </aerodynamics>
    </fdm_config>"#,
);

let mut fdm = Fdm::new()?;
fdm.load_model(&files, "aircraft", "box")?;
fdm.set_property("ic/h-sl-ft", 1000.0)?;
fdm.run_ic()?;

// Look a property up once, then read it by id every frame.
let altitude = fdm.node("position/h-sl-ft")?.expect("bound by the model");
for _ in 0..120 {
    fdm.run()?;
}
assert!(fdm.read_node(altitude)? < 1000.0);
# Ok::<(), jsbsim::Error>(())
```

With the `fs` feature, `FileResolver::new("path/to/jsbsim")` serves a JSBSim
data tree instead, and `Fdm::load_script` runs a script from it.
`load_model` reads `aircraft/box/box.xml`; for a model kept directly in its
directory, such as PX4's `models/Rascal/Rascal110-JSBSim.xml`, use
`Fdm::load_model_in_dir(&files, "models/Rascal", "Rascal110-JSBSim")`.

## Terrain

The host owns the terrain. A `GroundCallback` answers where the ground is
below a point; install it before loading so that initial conditions given
above ground are placed on it.

```rust
use jsbsim::math::Location;
use jsbsim::{Contact, Fdm, GroundCallback};

/// A sphere with no terrain on it.
struct Sphere {
    radius_ft: f64,
}

impl GroundCallback for Sphere {
    fn ag_level(&self, _time: f64, location: &Location) -> Contact {
        let r = location.radius();
        let mut normal = location.ecef();
        normal.normalize();
        let mut point = location.scale(self.radius_ft / r);
        point.set_ellipse(self.radius_ft, self.radius_ft);
        point.cache_derived();
        Contact::new(r - self.radius_ft, point, normal)
    }
}

let mut fdm = Fdm::new()?;
fdm.set_ground_callback(Box::new(Sphere { radius_ft: 20_925_646.3 }));
# Ok::<(), jsbsim::Error>(())
```

## Snapshots

A `Snapshot` holds the whole dynamic state of an executive, for rewind,
replay or lockstep. It restores only onto an executive that loaded the same
model the same way, and its bytes are the same on every target.

```rust
use jsbsim::snapshot::Snapshot;
use jsbsim::{Fdm, MemoryResolver};

# let mut files = MemoryResolver::new();
# files.insert(
#     "aircraft/box/box.xml",
#     r#"<fdm_config name="box" version="2.0">
#       <metrics> <wingarea unit="FT2"> 1 </wingarea> <wingspan unit="FT"> 1 </wingspan>
#         <chord unit="FT"> 1 </chord> </metrics>
#       <mass_balance> <ixx> 10 </ixx> <iyy> 10 </iyy> <izz> 10 </izz>
#         <emptywt> 100 </emptywt> <location name="CG"> <x>0</x> <y>0</y> <z>0</z> </location>
#       </mass_balance>
#       <ground_reactions> <contact type="STRUCTURE" name="BOTTOM">
#         <location> <x>0</x> <y>0</y> <z>-10</z> </location>
#         <spring_coeff> 1000 </spring_coeff> <damping_coeff> 100 </damping_coeff>
#       </contact> </ground_reactions>
#       <aerodynamics> <axis name="DRAG"/> </aerodynamics>
#     </fdm_config>"#,
# );
let mut fdm = Fdm::new()?;
fdm.load_model(&files, "aircraft", "box")?;
fdm.run_ic()?;

let mut saved = Snapshot::new();
fdm.snapshot_into(&mut saved)?;
let then = fdm.get_property("position/h-sl-ft")?;
for _ in 0..60 {
    fdm.run()?;
}
fdm.restore(&saved)?;
assert_eq!(fdm.get_property("position/h-sl-ft")?, then);

// The bytes can be stored or sent, then decoded again.
let copy = Snapshot::from_bytes(saved.as_bytes())?;
fdm.restore(&copy)?;
# Ok::<(), jsbsim::Error>(())
```

## Licence

LGPL-2.1-or-later, the licence of JSBSim; see `LICENSE`. The geodesic code is
derived from GeographicLib and is also under the MIT licence
(`LICENSES/GeographicLib-MIT.txt`); the elementary functions are derived from
ARM optimized-routines and are also under MIT OR Apache-2.0 WITH
LLVM-exception (`LICENSES/optimized-routines.txt`).
