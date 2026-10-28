// SPDX-License-Identifier: LGPL-2.1-or-later
use super::component::{Base, Tick};
use crate::fdm::Fdm;
use crate::{Error, LoadOptions, MemoryResolver};

/// An aircraft with nothing but `system` (a `<system>` element) to run.
#[cfg(test)]
pub(super) fn load_system(system: &str, options: LoadOptions) -> Result<Fdm, Error> {
    let aircraft = format!(
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
    </mass_balance>
    <ground_reactions/>
    {system}
</fdm_config>"#
    );
    let mut fdm = Fdm::with_options(options)?;
    // No <aerodynamics>: they would refuse to run.
    fdm.set_property("simulation/models/FGAerodynamics/enabled", 0.0)?;
    let mut resolver = MemoryResolver::new();
    resolver.insert("aircraft/t/t.xml", aircraft.into_bytes());
    fdm.load_model(&resolver, "aircraft", "t")?;
    Ok(fdm)
}

/// The error loading `system` fails with.
#[cfg(test)]
pub(super) fn load_error(system: &str, options: LoadOptions) -> Error {
    match load_system(system, options) {
        Ok(_) => panic!("loaded: {system}"),
        Err(e) => e,
    }
}

const COUNTER: &str = r#"<system>
  <channel name="slow" execrate="4">
    <fcs_function name="test/counter">
      <function>
        <sum> <property>test/counter</property> <value>1</value> </sum>
      </function>
    </fcs_function>
  </channel>
</system>"#;

#[test]
fn a_slow_channel_runs_every_frame_while_trimming() {
    let mut fdm = load_system(COUNTER, LoadOptions::default()).unwrap();
    // RunIC runs it twice with integration suspended; then it waits out
    // its rate.
    fdm.run_ic().unwrap();
    assert_eq!(fdm.get_property("test/counter").unwrap(), 2.0);
    for expected in [2.0, 2.0, 2.0, 3.0] {
        fdm.run().unwrap();
        assert_eq!(fdm.get_property("test/counter").unwrap(), expected);
    }
    fdm.models.exec.trim_status = true;
    for expected in [4.0, 5.0, 6.0] {
        fdm.run().unwrap();
        assert_eq!(fdm.get_property("test/counter").unwrap(), expected);
    }
}

#[test]
fn the_execute_property_must_exist() {
    let system = COUNTER.replace("execrate=\"4\"", "execute=\"test/missing\"");
    let err = load_error(&system, LoadOptions::default());
    assert!(
        matches!(&err, Error::Model { message } if message.contains("The On/Off property, test/missing")),
        "{err:?}"
    );
}

#[test]
fn unported_components_are_refused_and_unknown_ones_skipped() {
    let actuator = "<system><channel name=\"c\"><actuator name=\"a\"/></channel></system>";
    let err = load_error(actuator, LoadOptions::default());
    assert!(
        matches!(&err, Error::Unsupported { construct, .. } if construct == "<actuator> FCS component"),
        "{err:?}"
    );
    let unknown = "<system><channel name=\"c\"><bogus name=\"a\"/></channel></system>";
    let fdm = load_system(unknown, LoadOptions::default()).unwrap();
    assert!(fdm.models.fcs.channels[0].components.is_empty());
    let strict = LoadOptions { strict: true };
    let err = load_error(unknown, strict);
    assert!(
        matches!(&err, Error::Unsupported { construct, .. } if construct == "unknown FCS component <bogus>"),
        "{err:?}"
    );
}

#[test]
fn a_component_without_a_function_is_refused() {
    let system = "<system><channel name=\"c\"><fcs_function name=\"f\"/></channel></system>";
    let err = load_error(system, LoadOptions::default());
    assert!(
        matches!(&err, Error::Model { message } if message.contains("should contain a \"function\"")),
        "{err:?}"
    );
}

fn base_with_delay(frames: usize) -> Base {
    Base {
        name: "d".to_owned(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        clip: None,
        delay: vec![0.0; frames],
        index: 0,
        dt: 0.01,
        input: 0.0,
        output: 0.0,
    }
}

#[test]
fn the_delay_holds_outputs_back_by_its_length_and_fills_while_trimming() {
    let mut base = base_with_delay(3);
    let run = Tick {
        clip: None,
        trim_status: false,
    };
    let mut seen = Vec::new();
    for v in [1.0, 2.0, 3.0, 4.0, 5.0] {
        base.output = v;
        base.delay(&run);
        seen.push(base.output);
    }
    assert_eq!(seen, [0.0, 0.0, 1.0, 2.0, 3.0]);
    base.output = 9.0;
    base.delay(&Tick {
        clip: None,
        trim_status: true,
    });
    assert_eq!((base.output, base.delay.as_slice()), (9.0, &[9.0; 3][..]));
    base.reset_past_states();
    assert_eq!((base.index, base.delay.as_slice()), (0, &[0.0; 3][..]));
}

#[test]
fn delays_convert_seconds_with_the_channel_step() {
    let system = r#"<system>
  <channel name="c" execrate="2">
    <fcs_function name="a"> <function><value>1</value></function>
      <delay> 0.04 </delay> </fcs_function>
    <fcs_function name="b"> <function><value>1</value></function>
      <delay type="frames"> 3 </delay> </fcs_function>
    <fcs_function name="c"> <function><value>1</value></function>
      <delay type="ticks"> 3 </delay> </fcs_function>
  </channel>
</system>"#;
    let fdm = load_system(system, LoadOptions::default()).unwrap();
    let lens: Vec<usize> = fdm.models.fcs.channels[0]
        .components
        .iter()
        .map(|c| c.base.delay.len())
        .collect();
    // 0.04 s / (2 / 120 s) truncates to 2 frames.
    assert_eq!(lens, [2, 3, 0]);
    assert_eq!(
        fdm.models.fcs.channels[0].components[0].base.dt,
        2.0 / 120.0
    );
}

/// The `Error::Model` message loading one component `body` in a channel
/// fails with.
#[cfg(test)]
fn component_error(body: &str) -> String {
    let system = format!("<system><channel name=\"c\">{body}</channel></system>");
    match load_error(&system, LoadOptions::default()) {
        Error::Model { message } => message,
        other => panic!("{other:?}"),
    }
}

#[test]
fn gains_refuse_what_jsbsim_throws_on() {
    for (body, expected) in [
        ("<pure_gain name=\"g\"/>", "Not enough <input> nodes"),
        (
            "<scheduled_gain name=\"g\"><input>x</input></scheduled_gain>",
            "A table must be provided",
        ),
        (
            "<aerosurface_scale name=\"g\"><input>x</input></aerosurface_scale>",
            "No range supplied",
        ),
        (
            "<aerosurface_scale name=\"g\"><input>x</input><range><max>1</max></range></aerosurface_scale>",
            "Maximum and minimum output values must be supplied",
        ),
        (
            "<pure_gain name=\"g\"><input>x</input><gain>1\n2</gain></pure_gain>",
            "must either contain a value number or a property name",
        ),
    ] {
        let message = component_error(body);
        assert!(message.contains(expected), "{body}: {message}");
    }
}

#[test]
fn a_deadband_needs_an_input_and_a_summer_does_not() {
    let message = component_error("<deadband name=\"d\"/>");
    assert!(message.contains("Not enough <input> nodes"), "{message}");
    let system =
        "<system><channel name=\"c\"><summer name=\"s\"><bias>2</bias></summer></channel></system>";
    let mut fdm = load_system(system, LoadOptions::default()).unwrap();
    fdm.run_ic().unwrap();
    assert_eq!(fdm.get_property("fcs/s").unwrap(), 2.0);
}
