// SPDX-License-Identifier: LGPL-2.1-or-later
use crate::fdm::Fdm;
use crate::{Error, LoadOptions, MemoryResolver};

const STRICT: LoadOptions = LoadOptions { strict: true };
const LENIENT: LoadOptions = LoadOptions { strict: false };

/// A box aircraft whose system declares the properties the `<input>`
/// functions copy to, followed by `input`.
fn aircraft(input: &str) -> String {
    format!(
        r#"<fdm_config name="box">
  <metrics/>
  <mass_balance>
    <ixx>10</ixx><iyy>10</iyy><izz>10</izz><emptywt>100</emptywt>
    <location name="CG" unit="IN"><x>0</x><y>0</y><z>0</z></location>
  </mass_balance>
  <ground_reactions/>
  <system name="declarations">
    <property value="0">test/counter</property>
    <property value="0">test/post</property>
  </system>
{input}
</fdm_config>"#
    )
}

/// A socket input counting the frames it runs by its declared gain, and
/// copying ten times the count after the socket read.
const COUNTER: &str = r#"  <input>
    <property value="2.5">test/gain</property>
    <function copyto="test/counter">
      <sum><property>test/counter</property><property>test/gain</property></sum>
    </function>
    <function type="post" copyto="test/post">
      <product><property>test/counter</property><value>10</value></product>
    </function>
  </input>"#;

fn load(options: LoadOptions, files: &[(&str, &str)]) -> Result<Fdm, Error> {
    let mut r = MemoryResolver::new();
    for (path, text) in files {
        r.insert(*path, *text);
    }
    let mut fdm = Fdm::with_options(options)?;
    // A box has no aerodynamic axes, which JSBSim refuses to run.
    fdm.set_property("simulation/models/FGAerodynamics/enabled", 0.0)?;
    fdm.load_model(&r, "aircraft", "box")?;
    Ok(fdm)
}

fn load_box(options: LoadOptions, input: &str) -> Result<Fdm, Error> {
    let text = aircraft(input);
    load(options, &[("aircraft/box/box.xml", &text)])
}

fn get(fdm: &mut Fdm, path: &str) -> f64 {
    fdm.get_property(path).unwrap()
}

#[test]
fn input_functions_run_every_frame_strict_or_not() {
    for options in [STRICT, LENIENT] {
        let mut fdm = load_box(options, COUNTER).unwrap();
        assert_eq!(get(&mut fdm, "test/gain"), 2.5);
        fdm.run_ic().unwrap();
        // `Initialize` and `RunIC` each run a frame.
        assert_eq!(get(&mut fdm, "test/counter"), 5.0);
        for _ in 0..3 {
            fdm.run().unwrap();
        }
        assert_eq!(get(&mut fdm, "test/counter"), 12.5);
        assert_eq!(get(&mut fdm, "test/post"), 125.0);
        // Holding does not stop `FGInput`.
        fdm.set_property("simulation/pause", 1.0).unwrap();
        fdm.run().unwrap();
        assert_eq!(get(&mut fdm, "test/counter"), 15.0);
    }
}

#[test]
fn input_functions_skip_trims_and_disabled_inputs() {
    let mut fdm = load_box(STRICT, COUNTER).unwrap();
    fdm.run_ic().unwrap();
    fdm.models.exec.trim_status = true;
    fdm.run().unwrap();
    fdm.models.exec.trim_status = false;
    assert_eq!(get(&mut fdm, "test/counter"), 5.0);
    fdm.set_property("simulation/models/FGInput/enabled", 0.0)
        .unwrap();
    fdm.run().unwrap();
    assert_eq!(get(&mut fdm, "test/counter"), 5.0);
    fdm.set_property("simulation/models/FGInput/enabled", 1.0)
        .unwrap();
    fdm.run().unwrap();
    assert_eq!(get(&mut fdm, "test/counter"), 7.5);
    // `DisableInput` also disables each type, so the model's flag alone
    // does not bring it back; `Toggle` does.
    fdm.disable_input();
    fdm.set_property("simulation/models/FGInput/enabled", 1.0)
        .unwrap();
    fdm.run().unwrap();
    assert_eq!(get(&mut fdm, "test/counter"), 7.5);
    assert!(fdm.models.input.toggle(0));
    fdm.run().unwrap();
    assert_eq!(get(&mut fdm, "test/counter"), 10.0);
    fdm.enable_input();
    fdm.run().unwrap();
    assert_eq!(get(&mut fdm, "test/counter"), 12.5);
}

#[test]
fn run_ic_restores_the_declared_properties() {
    let mut fdm = load_box(STRICT, COUNTER).unwrap();
    fdm.set_property("test/gain", 1.0).unwrap();
    fdm.run_ic().unwrap();
    // `Initialize` runs with the written gain, `RunIC` after the reset.
    assert_eq!(get(&mut fdm, "test/counter"), 3.5);
    assert_eq!(get(&mut fdm, "test/gain"), 2.5);
}

#[test]
fn a_udp_input_runs_at_its_rate() {
    let input = COUNTER.replacen(
        "<input>",
        r#"<input type="QTJSBSIM" port="5139" rate="40">"#,
        1,
    );
    let mut fdm = load_box(STRICT, &input).unwrap();
    // 0.5 + 1/(dt*40) at 120 Hz: every third frame.
    assert_eq!(fdm.models.input.types[0].base.rate, 3);
    fdm.run_ic().unwrap();
    let mut counts = Vec::new();
    for _ in 0..6 {
        fdm.run().unwrap();
        counts.push(get(&mut fdm, "test/counter"));
    }
    assert_eq!(counts, [5.0, 5.0, 7.5, 7.5, 7.5, 10.0]);
}

#[test]
fn a_udp_input_without_a_port_keeps_the_frame_rate() {
    let input = COUNTER.replacen("<input>", r#"<input type="qtjsbsim" rate="40">"#, 1);
    let fdm = load_box(STRICT, &input).unwrap();
    assert_eq!(fdm.models.input.types[0].base.rate, 1);
}

#[test]
fn input_types_jsbsim_has_no_class_for_fail_the_load() {
    for kind in ["NONE", "none", "bogus"] {
        for options in [STRICT, LENIENT] {
            let input = format!(r#"<input type="{kind}"/>"#);
            match load_box(options, &input) {
                Err(Error::UnknownInputType { kind: k, path }) => {
                    assert_eq!(k, kind);
                    assert!(path.contains("box.xml"), "{path}");
                }
                other => panic!("{kind}: expected UnknownInputType, got {:?}", other.err()),
            }
        }
    }
}

#[test]
fn socket_only_inputs_load_strictly() {
    for input in [
        r#"<input port="1137"/>"#,
        r#"<input type="SOCKET" port="1137" action="BLOCKING_INPUT"/>"#,
        "<input/>",
    ] {
        let mut fdm = load_box(STRICT, input).unwrap();
        fdm.run_ic().unwrap();
        fdm.run().unwrap();
        assert_eq!(fdm.models.input.types.len(), 1);
    }
}

#[test]
fn a_named_input_function_fails_as_its_second_binding_does() {
    let input = r#"<input><function name="test/named"><value>1</value></function></input>"#;
    for options in [STRICT, LENIENT] {
        let err = load_box(options, input).err();
        assert!(
            matches!(
                err,
                Some(Error::Function(
                    crate::functions::FunctionError::AlreadyBound { .. }
                ))
            ),
            "{err:?}"
        );
    }
}

#[test]
fn an_included_input_replaces_the_local_element() {
    let text = aircraft(
        r#"<input file="in.xml" type="NONE"><property value="1">test/local</property></input>"#,
    );
    let included = COUNTER.trim();
    let mut fdm = load(
        STRICT,
        &[
            ("aircraft/box/box.xml", &text),
            ("aircraft/box/in.xml", included),
        ],
    )
    .unwrap();
    assert!(fdm.node("test/local").unwrap().is_none());
    fdm.run_ic().unwrap();
    assert_eq!(get(&mut fdm, "test/counter"), 5.0);
}

#[test]
fn strict_still_refuses_what_jsbsim_ignores() {
    let input = r#"<system><channel name="c"><bogus name="a"/></channel></system>"#;
    let err = load_box(STRICT, input).err();
    assert!(
        matches!(&err, Some(Error::Unsupported { construct, .. }) if construct == "unknown FCS component <bogus>"),
        "{err:?}"
    );
    load_box(LENIENT, input).unwrap();
}

#[test]
fn outputs_load_strictly() {
    let output = r#"<output name="out.csv" type="CSV" rate="10">
    <function name="test/out"><value>3</value></function>
  </output>"#;
    let mut fdm = load_box(STRICT, output).unwrap();
    assert_eq!(fdm.models.output.types.len(), 1);
    assert_eq!(get(&mut fdm, "test/out"), 3.0);
}

#[test]
fn an_input_copies_to_the_properties_it_declares_strict_or_not() {
    let input = r#"<input>
    <property value="0">test/own</property>
    <function copyto="test/own">
      <sum><property>test/own</property><value>1</value></sum>
    </function>
    <function type="post" copyto="test/own">
      <product><property>test/own</property><value>2</value></product>
    </function>
  </input>"#;
    for options in [STRICT, LENIENT] {
        let mut fdm = load_box(options, input).unwrap();
        fdm.run_ic().unwrap();
        fdm.run().unwrap();
        // `RunIC` resets it to 0, then two frames of (x + 1) * 2.
        assert_eq!(get(&mut fdm, "test/own"), 6.0);
    }
}
