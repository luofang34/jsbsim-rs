// SPDX-License-Identifier: LGPL-2.1-or-later
//! Supplies the terrain from the host: a plateau, then a deck that moves.
//!
//! `cargo run -p jsbsim --example terrain`

use jsbsim::math::{ColumnVector3, Location};
use jsbsim::{Contact, Error, Fdm, GroundCallback, MemoryResolver};

/// The Earth's semi-axes in feet, as JSBSim's default planet has them.
const EARTH_A: f64 = 20_925_646.325_46;
const EARTH_B: f64 = 20_855_486.595_1;

const BOX: &str = r#"<fdm_config name="box" version="2.0">
  <metrics> <wingarea unit="FT2"> 1 </wingarea> <wingspan unit="FT"> 1 </wingspan>
    <chord unit="FT"> 1 </chord> </metrics>
  <mass_balance> <ixx> 10 </ixx> <iyy> 10 </iyy> <izz> 10 </izz>
    <emptywt> 100 </emptywt> <location name="CG"> <x>0</x> <y>0</y> <z>0</z> </location>
  </mass_balance>
  <ground_reactions>
  <contact type="STRUCTURE" name="FL">
    <location unit="FT"> <x>1</x> <y>-1</y> <z>-1</z> </location>
    <static_friction> 0.8 </static_friction> <dynamic_friction> 0.5 </dynamic_friction>
    <spring_coeff> 1000 </spring_coeff> <damping_coeff> 100 </damping_coeff>
  </contact>
  <contact type="STRUCTURE" name="FR">
    <location unit="FT"> <x>1</x> <y>1</y> <z>-1</z> </location>
    <static_friction> 0.8 </static_friction> <dynamic_friction> 0.5 </dynamic_friction>
    <spring_coeff> 1000 </spring_coeff> <damping_coeff> 100 </damping_coeff>
  </contact>
  <contact type="STRUCTURE" name="BL">
    <location unit="FT"> <x>-1</x> <y>-1</y> <z>-1</z> </location>
    <static_friction> 0.8 </static_friction> <dynamic_friction> 0.5 </dynamic_friction>
    <spring_coeff> 1000 </spring_coeff> <damping_coeff> 100 </damping_coeff>
  </contact>
  <contact type="STRUCTURE" name="BR">
    <location unit="FT"> <x>-1</x> <y>1</y> <z>-1</z> </location>
    <static_friction> 0.8 </static_friction> <dynamic_friction> 0.5 </dynamic_friction>
    <spring_coeff> 1000 </spring_coeff> <damping_coeff> 100 </damping_coeff>
  </contact>
  </ground_reactions>
  <aerodynamics> <axis name="DRAG"/> </aerodynamics>
</fdm_config>"#;

// ANCHOR: plateau
/// Level ground a fixed height above a spherical Earth.
struct Plateau {
    radius_ft: f64,
}

impl GroundCallback for Plateau {
    fn ag_level(&self, _time: f64, location: &Location) -> Contact {
        // Read only the ECEF position: the location may carry no ellipse.
        let r = location.radius();
        let mut normal = location.ecef();
        normal.normalize();
        // The ground point straight below, with the planet's shape set so
        // that JSBSim can take its geodetic altitude.
        let mut point = *location * (self.radius_ft / r);
        point.set_ellipse(EARTH_A, EARTH_B);
        point.cache_derived();
        Contact::new(r - self.radius_ft, point, normal)
    }
}
// ANCHOR_END: plateau

// ANCHOR: deck
/// A flat deck tangent to the equator at longitude 0, sliding north.
struct Deck {
    height_ft: f64,
    north_fps: f64,
}

impl GroundCallback for Deck {
    fn ag_level(&self, _time: f64, location: &Location) -> Contact {
        // At (0°, 0°) the local up is ECEF x and north is ECEF z.
        let up = ColumnVector3::new(1.0, 0.0, 0.0);
        let r = location.ecef();
        let agl = r.dot(up) - (EARTH_A + self.height_ft);
        let mut point = Location::from_ecef(r - agl * up);
        point.set_ellipse(EARTH_A, EARTH_B);
        point.cache_derived();
        // The surface velocity at the queried point; a rotating deck would
        // add its angular velocity crossed with the point's offset.
        let velocity = ColumnVector3::new(0.0, 0.0, self.north_fps);
        Contact::new(agl, point, up).with_motion(velocity, ColumnVector3::zero())
    }
}
// ANCHOR_END: deck

/// Drops the box from `agl` feet above whatever terrain `ground` gives, at
/// (0°, 0°), and lets it come to rest.
fn drop_onto(ground: Box<dyn GroundCallback>, agl: f64) -> Result<Fdm, Error> {
    let mut files = MemoryResolver::new();
    files.insert("aircraft/box/box.xml", BOX);
    // ANCHOR: install
    let mut fdm = Fdm::new()?;
    // Before loading, so that initial conditions above ground use it.
    fdm.set_ground_callback(ground);
    fdm.load_model(&files, "aircraft", "box")?;
    fdm.set_property("ic/lat-geod-deg", 0.0)?;
    fdm.set_property("ic/long-gc-deg", 0.0)?;
    fdm.set_property("ic/h-agl-ft", agl)?;
    fdm.run_ic()?;
    // ANCHOR_END: install
    while fdm.sim_time() < 10.0 {
        fdm.run()?;
    }
    Ok(fdm)
}

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().try_init().ok();

    let mut fdm = drop_onto(
        Box::new(Plateau {
            radius_ft: EARTH_A + 500.0,
        }),
        50.0,
    )?;
    let terrain = fdm.get_property("position/terrain-elevation-asl-ft")?;
    let agl = fdm.get_property("position/h-agl-ft")?;
    tracing::info!("on the plateau: terrain at {terrain:.1} ft, CG {agl:.2} ft above it");
    assert!((terrain - 500.0).abs() < 1.0, "terrain {terrain}");
    assert!((0.5..1.5).contains(&agl), "agl {agl}");

    let mut fdm = drop_onto(
        Box::new(Deck {
            height_ft: 30.0,
            north_fps: 20.0,
        }),
        5.0,
    )?;
    let north = fdm.get_property("velocities/v-north-fps")?;
    tracing::info!("resting on the deck, moving north at {north:.2} ft/s");
    assert!((north - 20.0).abs() < 0.5, "north {north}");
    Ok(())
}

#[test]
fn runs() -> Result<(), Error> {
    main()
}
