# jsbsim-rs

[![CI](https://github.com/luofang34/jsbsim-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/luofang34/jsbsim-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/jsbsim.svg)](https://crates.io/crates/jsbsim)
[![Manual](https://img.shields.io/badge/manual-mdBook-blue)](https://luofang34.github.io/jsbsim-rs/)

An **unofficial** pure-Rust port of [JSBSim](https://github.com/JSBSim-Team/jsbsim),
the open-source flight dynamics model. It is not affiliated with or endorsed by
the JSBSim project.

The `jsbsim` crate has no C++ dependency, does no I/O of its own, is
deterministic, and builds for native targets and for `wasm32-unknown-unknown`.

**Read the [user manual](https://luofang34.github.io/jsbsim-rs/)** for
everything beyond this page: flying JSBSim aircraft and scripts, properties,
trimming, terrain, snapshots, what is supported, and how the port is verified
against C++ JSBSim and developed.

## Quickstart

```sh
cargo add jsbsim
```

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

To fly JSBSim's Cessna 172 from this repository's copy of JSBSim's files:

```sh
cargo run -p jsbsim --features fs --release --example fly_c172
```

To build and run every CI gate locally: `scripts/ci.sh` (see
[Building, testing and CI](https://luofang34.github.io/jsbsim-rs/development.html)).

## Licence

LGPL-2.1-or-later, the licence of JSBSim; see [LICENSE](LICENSE). Source files
derived from JSBSim keep the copyright lines of the C++ files they translate and
name those files. Parts are also under MIT and Apache-2.0 terms; see the
[licence page](https://luofang34.github.io/jsbsim-rs/licence.html).
