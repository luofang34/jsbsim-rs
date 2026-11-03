// SPDX-License-Identifier: LGPL-2.1-or-later
//! What loading reports when files are missing, malformed, or use what the
//! port lacks, and what strict mode changes.
//!
//! `cargo run -p jsbsim --example errors`

use jsbsim::error::XmlError;
use jsbsim::{Error, Fdm, LoadOptions, MemoryResolver};

const BODY: &str = r#"
  <metrics> <wingarea unit="FT2"> 1 </wingarea> <wingspan unit="FT"> 1 </wingspan>
    <chord unit="FT"> 1 </chord> </metrics>
  <mass_balance> <ixx> 10 </ixx> <iyy> 10 </iyy> <izz> 10 </izz>
    <emptywt> 100 </emptywt> <location name="CG"> <x>0</x> <y>0</y> <z>0</z> </location>
  </mass_balance>
  <ground_reactions> <contact type="STRUCTURE" name="BOTTOM">
    <location> <x>0</x> <y>0</y> <z>-10</z> </location>
    <spring_coeff> 1000 </spring_coeff> <damping_coeff> 100 </damping_coeff>
  </contact> </ground_reactions>
  <aerodynamics> <axis name="DRAG"/> </aerodynamics>"#;

/// A box aircraft with `extra` inside its `<fdm_config>`.
fn files_with(extra: &str) -> MemoryResolver {
    let mut files = MemoryResolver::new();
    files.insert(
        "aircraft/box/box.xml",
        format!(r#"<fdm_config name="box" version="2.0">{BODY}{extra}</fdm_config>"#),
    );
    files
}

fn load(options: LoadOptions, files: &MemoryResolver) -> Result<Fdm, Error> {
    let mut fdm = Fdm::with_options(options)?;
    fdm.load_model(files, "aircraft", "box")?;
    Ok(fdm)
}

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().try_init().ok();
    let lenient = LoadOptions::new();

    // ANCHOR: report
    match load(
        lenient,
        &files_with("<flight_control> <oops> </flight_control>"),
    ) {
        Ok(_) => tracing::info!("loaded"),
        Err(error) => {
            // Errors from a file say where: "In file <path>: line <n>".
            match error.location() {
                Some(at) => tracing::error!("{error} ({}:{})", at.file, at.line),
                None => tracing::error!("{error}"),
            }
        }
    }
    // ANCHOR_END: report

    // ANCHOR: kinds
    let mut fdm = Fdm::new()?;
    let missing = fdm.load_model(&MemoryResolver::new(), "aircraft", "nothing");
    assert!(matches!(missing, Err(Error::ResourceNotFound { .. })));

    let malformed = load(lenient, &files_with("<flight_control>"));
    assert!(matches!(
        malformed,
        Err(Error::Xml(XmlError::Malformed { .. }))
    ));

    // What the port does not implement fails the load, strict or not.
    let unported = load(lenient, &files_with(r#"<child name="other"/>"#));
    assert!(matches!(unported, Err(Error::Unsupported { .. })));
    // ANCHOR_END: kinds

    // ANCHOR: strict
    // JSBSim itself logs and skips an FCS component it does not know...
    let unknown = r#"<flight_control name="fcs"> <channel name="c">
        <warp_drive name="fcs/engage"/> </channel> </flight_control>"#;
    assert!(load(lenient, &files_with(unknown)).is_ok());
    // ...which strict mode refuses.
    let strict = LoadOptions::new().with_strict(true);
    let refused = load(strict, &files_with(unknown));
    assert!(matches!(refused, Err(Error::Unsupported { .. })));
    // ANCHOR_END: strict
    Ok(())
}

#[test]
fn runs() -> Result<(), Error> {
    main()
}
