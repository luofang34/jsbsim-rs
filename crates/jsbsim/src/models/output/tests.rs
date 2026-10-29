// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use crate::fdm::Fdm;
use crate::{LoadOptions, MemoryResolver};

const DT: f64 = 1.0 / 120.0;

#[test]
fn the_rate_is_a_frame_count_read_back_as_hz() {
    let mut t = OutputType::default();
    t.set_rate_hz(10.0, DT);
    assert_eq!((t.rate, t.enabled), (12, true));
    assert_eq!(t.rate_hz(DT), 1.0 / (12.0 * DT));
    // 7 Hz at 120 Hz is 17 frames, which reads back as 7.06 Hz.
    t.set_rate_hz(7.0, DT);
    assert_eq!(t.rate, 17);
    assert_eq!(t.rate_hz(DT), 1.0 / (17.0 * DT));
    // The frame count is kept when dt changes; the rate in Hz follows.
    assert_eq!(t.rate_hz(DT / 2.0), 1.0 / (17.0 * DT / 2.0));
}

#[test]
fn rates_clamp_and_zero_or_negative_disables() {
    let mut t = OutputType::default();
    // Clamped to 1000 Hz, faster than the frame rate: no frames between
    // prints, which reads back as infinitely fast.
    t.set_rate_hz(5000.0, DT);
    assert_eq!((t.rate, t.enabled), (0, true));
    assert_eq!(t.rate_hz(DT), f64::INFINITY);
    t.set_rate_hz(0.0, DT);
    assert_eq!((t.rate, t.enabled), (1, false));
    t.set_rate_hz(10.0, DT);
    t.set_rate_hz(-3.0, DT);
    assert_eq!((t.rate, t.enabled), (1, false));
    t.set_rate_hz(f64::NAN, DT);
    assert_eq!((t.rate, t.enabled), (1, false));
}

fn aircraft(outputs: &str) -> String {
    format!(
        r#"<fdm_config name="T" version="2.0">
    <metrics>
        <wingarea unit="FT2"> 1 </wingarea>
        <wingspan unit="FT"> 1 </wingspan>
        <chord unit="FT"> 1 </chord>
    </metrics>
    <mass_balance>
        <ixx unit="SLUG*FT2"> 10 </ixx>
        <iyy unit="SLUG*FT2"> 10 </iyy>
        <izz unit="SLUG*FT2"> 10 </izz>
        <emptywt unit="LBS"> 100 </emptywt>
        <location name="CG" unit="IN"> <x> 0 </x> <y> 0 </y> <z> 0 </z> </location>
    </mass_balance>
    <ground_reactions/>
    {outputs}
</fdm_config>"#
    )
}

fn load(outputs: &str) -> Result<Fdm, Error> {
    let mut fdm = Fdm::with_options(LoadOptions::default())?;
    let mut resolver = MemoryResolver::new();
    resolver.insert("aircraft/t/t.xml", aircraft(outputs).into_bytes());
    fdm.load_model(&resolver, "aircraft", "t")?;
    Ok(fdm)
}

const TWO_OUTPUTS: &str = r#"
    <output name="a.csv" type="CSV" rate="10">
        <rates> ON </rates>
        <property> velocities/u-fps </property>
    </output>
    <output name="localhost" type="socket" port="1138"/>
"#;

#[test]
fn each_output_ties_its_rate_and_enable_flag() {
    let mut fdm = load(TWO_OUTPUTS).unwrap();
    let dt = fdm.models.exec.dt;
    assert_eq!(
        fdm.get_property("simulation/output/log_rate_hz").unwrap(),
        1.0 / (f64::from((0.5 + 1.0 / (dt * 10.0)) as u32) * dt)
    );
    assert_eq!(
        fdm.get_property("simulation/output[0]/enabled").unwrap(),
        1.0
    );
    // No rate attribute: 1 Hz.
    assert_eq!(
        fdm.get_property("simulation/output[1]/log_rate_hz")
            .unwrap(),
        1.0 / (f64::from((0.5 + 1.0 / dt) as u32) * dt)
    );
    assert!(fdm.node("simulation/output[2]/enabled").unwrap().is_none());
}

#[test]
fn writes_are_stored_and_read_back_as_jsbsim_does() {
    let mut fdm = load(TWO_OUTPUTS).unwrap();
    let dt = fdm.models.exec.dt;
    assert!(
        fdm.set_property("simulation/output[1]/log_rate_hz", 7.0)
            .unwrap()
    );
    let frames = f64::from((0.5 + 1.0 / (dt * 7.0)) as u32);
    assert_eq!(
        fdm.get_property("simulation/output[1]/log_rate_hz")
            .unwrap(),
        1.0 / (frames * dt)
    );
    fdm.set_property("simulation/output[1]/log_rate_hz", 0.0)
        .unwrap();
    assert_eq!(
        fdm.get_property("simulation/output[1]/enabled").unwrap(),
        0.0
    );
    fdm.set_property("simulation/output/enabled", 0.0).unwrap();
    assert!(!fdm.models.output.types[0].enabled);
    fdm.set_property("simulation/output/enabled", 2.0).unwrap();
    assert_eq!(fdm.get_property("simulation/output/enabled").unwrap(), 1.0);
}

#[test]
fn outputs_change_nothing_else() {
    let mut with = load(TWO_OUTPUTS).unwrap();
    let mut without = load("").unwrap();
    for fdm in [&mut with, &mut without] {
        fdm.set_property("simulation/models/FGAerodynamics/enabled", 0.0)
            .unwrap();
        fdm.run_ic().unwrap();
        fdm.set_property("simulation/output/log_rate_hz", 120.0)
            .ok();
        for _ in 0..30 {
            fdm.run().unwrap();
        }
    }
    for path in ["position/h-sl-ft", "velocities/w-fps", "attitude/theta-rad"] {
        assert_eq!(
            with.get_property(path).unwrap().to_bits(),
            without.get_property(path).unwrap().to_bits(),
            "{path}"
        );
    }
}

#[test]
fn types_jsbsim_cannot_make_fail_the_load() {
    for kind in ["TERMINAL", "NONE", "PRINTER", ""] {
        let output = format!(r#"<output name="x" type="{kind}"/>"#);
        assert!(matches!(load(&output), Err(Error::Model { .. })), "{kind}");
    }
}

fn load_with(outputs: &str, files: &[(&str, &str)]) -> Result<Fdm, Error> {
    let mut fdm = Fdm::with_options(LoadOptions::default())?;
    let mut resolver = MemoryResolver::new();
    resolver.insert("aircraft/t/t.xml", aircraft(outputs).into_bytes());
    for (path, text) in files {
        resolver.insert(*path, text.as_bytes().to_vec());
    }
    fdm.load_model(&resolver, "aircraft", "t")?;
    Ok(fdm)
}

#[test]
fn an_included_output_file_adds_its_attributes() {
    let output = r#"<output name="x" file="more.xml"/>"#;
    let more = r#"<output type="CSV" rate="5"><property>velocities/u-fps</property></output>"#;
    let mut fdm = load_with(output, &[("aircraft/t/more.xml", more)]).unwrap();
    let dt = fdm.models.exec.dt;
    assert_eq!(
        fdm.get_property("simulation/output/log_rate_hz").unwrap(),
        1.0 / (f64::from((0.5 + 1.0 / (dt * 5.0)) as u32) * dt)
    );
}

#[test]
fn an_output_file_that_is_missing_or_not_an_output_fails() {
    let output = r#"<output name="x" type="CSV" file="more.xml"/>"#;
    assert!(load_with(output, &[]).is_err());
    let other = r#"<input type="CSV"/>"#;
    let err = load_with(output, &[("aircraft/t/more.xml", other)]).err();
    assert!(matches!(err, Some(Error::Model { .. })), "{err:?}");
}
