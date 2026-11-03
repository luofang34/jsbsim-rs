// SPDX-License-Identifier: LGPL-2.1-or-later
//! Serves model files from memory, and from a resolver of one's own that
//! lays local edits over a shared tree.
//!
//! `cargo run -p jsbsim --example resolvers`

use jsbsim::{Error, Fdm, MemoryResolver, Resolver};

const BOX: &str = r#"<fdm_config name="box" version="2.0">
  <metrics> <wingarea unit="FT2"> 1 </wingarea> <wingspan unit="FT"> 1 </wingspan>
    <chord unit="FT"> 1 </chord> </metrics>
  <mass_balance> <ixx> 10 </ixx> <iyy> 10 </iyy> <izz> 10 </izz>
    <emptywt> 100 </emptywt> <location name="CG"> <x>0</x> <y>0</y> <z>0</z> </location>
  </mass_balance>
  <ground_reactions> <contact type="STRUCTURE" name="BOTTOM">
    <location> <x>0</x> <y>0</y> <z>-10</z> </location>
    <spring_coeff> 1000 </spring_coeff> <damping_coeff> 100 </damping_coeff>
  </contact> </ground_reactions>
  <system file="beacon"/>
  <aerodynamics> <axis name="DRAG"/> </aerodynamics>
</fdm_config>"#;

/// A system the aircraft includes by name: a light that goes out at `off_s`.
fn beacon(off_s: f64) -> String {
    format!(
        r#"<system name="beacon">
      <channel name="lights">
        <fcs_function name="lights/beacon">
          <function> <lt>
            <property>simulation/sim-time-sec</property> <value>{off_s}</value>
          </lt> </function>
        </fcs_function>
      </channel>
    </system>"#
    )
}

// ANCHOR: overlay
/// Serves `local` where it has a file and `shared` everywhere else, the
/// way a user's edits might sit over a read-only aircraft collection.
struct Overlay<'a> {
    local: &'a dyn Resolver,
    shared: &'a dyn Resolver,
}

impl Resolver for Overlay<'_> {
    fn resolve(&self, path: &str) -> Result<Vec<u8>, Error> {
        match self.local.resolve(path) {
            Err(Error::ResourceNotFound { .. }) => self.shared.resolve(path),
            found => found,
        }
    }

    fn exists(&self, path: &str) -> bool {
        self.local.exists(path) || self.shared.exists(path)
    }
}
// ANCHOR_END: overlay

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().try_init().ok();

    // ANCHOR: memory
    // Paths are relative to the resolver's root. `<system file="beacon"/>`
    // is looked for in the aircraft's directory, its Systems/ and systems/
    // directories, then in the systems path, "systems" by default.
    let mut shared = MemoryResolver::new();
    shared.insert("aircraft/box/box.xml", BOX);
    shared.insert("systems/beacon.xml", beacon(0.5));
    // ANCHOR_END: memory

    let mut stock = Fdm::new()?;
    stock.load_model(&shared, "aircraft", "box")?;
    stock.run_ic()?;

    // ANCHOR: overlay_use
    // A local copy of the system in the aircraft's own directory wins the
    // search; the aircraft itself still comes from the shared tree.
    let mut local = MemoryResolver::new();
    local.insert("aircraft/box/Systems/beacon.xml", beacon(4.0));
    let files = Overlay {
        local: &local,
        shared: &shared,
    };
    let mut edited = Fdm::new()?;
    edited.load_model(&files, "aircraft", "box")?;
    edited.run_ic()?;
    // ANCHOR_END: overlay_use

    // At 0.75 s the stock beacon is out, the edited one still lit.
    while stock.sim_time() < 0.75 {
        stock.run()?;
        edited.run()?;
    }
    let (a, b) = (
        stock.get_property("lights/beacon")?,
        edited.get_property("lights/beacon")?,
    );
    tracing::info!(
        "at {:.2} s: stock beacon {a}, edited beacon {b}",
        stock.sim_time()
    );
    assert_eq!((a, b), (0.0, 1.0));
    Ok(())
}

#[test]
fn runs() -> Result<(), Error> {
    main()
}
