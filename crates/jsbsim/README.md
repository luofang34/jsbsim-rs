# jsbsim

An **unofficial** pure-Rust port of [JSBSim](https://github.com/JSBSim-Team/jsbsim),
the open-source flight dynamics model. It is not affiliated with or endorsed by
the JSBSim project.

The crate has no C++ dependency and performs no I/O of its own: aircraft,
engine, system and script files reach it through a `Resolver`. It is
deterministic and builds for native targets and for `wasm32-unknown-unknown`.
The `fs` feature adds `FileResolver`, which reads them from a directory.

**[User manual](https://luofang34.github.io/jsbsim-rs/)**: loading models
and scripts, initial conditions, properties, trimming, linearization,
terrain, snapshots, determinism and WebAssembly, and what of JSBSim is
supported.

## Quickstart

Drop a box from 1000 ft:

```rust
use jsbsim::{Fdm, MemoryResolver};

fn main() -> Result<(), jsbsim::Error> {
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
    while fdm.sim_time() < 5.0 {
        fdm.run()?;
    }
    assert!(fdm.get_property("position/h-sl-ft")? < 700.0);
    Ok(())
}
```

With the `fs` feature, `FileResolver::new("path/to/jsbsim")` serves a JSBSim
data tree instead, and `Fdm::load_script` runs one of its scripts.

## Licence

LGPL-2.1-or-later, the licence of JSBSim; see `LICENSE`. The geodesic code is
derived from GeographicLib and is also under the MIT licence
(`LICENSES/GeographicLib-MIT.txt`); the elementary functions are derived from
ARM optimized-routines and are also under MIT OR Apache-2.0 WITH
LLVM-exception (`LICENSES/optimized-routines.txt`).
