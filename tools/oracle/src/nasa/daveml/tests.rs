// SPDX-License-Identifier: LGPL-2.1-or-later

use std::collections::BTreeMap;

use jsbsim::oracle_support::xml::parse_document;

use super::{Model, shots};

const MODEL: &str = r#"<?xml version="1.0" standalone="no"?>
<!DOCTYPE DAVEfunc PUBLIC "-//AIAA//DTD for Flight Dynamic Models - Functions 2.0//EN"
          "http://www.daveml.org/DTDs/2p0/DAVEfunc.dtd">
<DAVEfunc xmlns="http://daveml.org/2010/DAVEML">
  <variableDef name="input" varID="x" units="nd"><description>an input</description></variableDef>
  <variableDef name="gain" varID="k" units="nd" initialValue="2.0"/>
  <variableDef name="clipped" varID="y" units="nd" minValue="-1" maxValue="3">
    <calculation>
      <math xmlns="http://www.w3.org/1998/Math/MathML">
        <apply><times/><ci>k</ci><ci>x</ci></apply>
      </math>
    </calculation>
  </variableDef>
  <variableDef name="switch" varID="s" units="nd">
    <calculation>
      <math xmlns="http://www.w3.org/1998/Math/MathML">
        <apply><piecewise>
          <piece><cn>10.</cn><apply><gt/><ci>x</ci><cn>1</cn></apply></piece>
          <otherwise><apply><minus/><ci>y</ci></apply></otherwise>
        </piecewise></apply>
      </math>
    </calculation>
  </variableDef>
  <variableDef name="angle" varID="a" units="rad">
    <calculation>
      <math xmlns="http://www.w3.org/1998/Math/MathML">
        <apply>
          <csymbol definitionURL="http://daveml.org/function_spaces.html#atan2" encoding="text">atan2</csymbol>
          <apply><divide/><ci>x</ci><cn>2</cn></apply>
          <apply><abs/><apply><minus/><cn>0</cn><ci>k</ci></apply></apply>
        </apply>
      </math>
    </calculation>
  </variableDef>
  <checkData>
    <staticShot name="one">
      <checkInputs>
        <signal><signalName>input</signalName><signalValue> 0.5</signalValue></signal>
      </checkInputs>
      <internalValues>
        <signal> <varID>y</varID> <signalValue>1.0</signalValue> </signal>
      </internalValues>
      <checkOutputs>
        <signal><signalName>clipped</signalName><signalValue>1.0</signalValue><tol>0.000001</tol></signal>
      </checkOutputs>
    </staticShot>
  </checkData>
</DAVEfunc>
"#;

fn model() -> Model {
    let root = parse_document(MODEL.as_bytes(), "m.dml").unwrap();
    Model::from_element("m.dml", &root).unwrap()
}

fn with_x(x: f64) -> BTreeMap<String, f64> {
    let v = model()
        .evaluate(&BTreeMap::from([("x".to_owned(), x)]))
        .unwrap();
    assert!(v.contains_key("a"));
    v
}

#[test]
fn calculations_initial_values_and_clips() {
    let v = with_x(0.5);
    assert_eq!(v["k"], 2.0);
    assert_eq!(v["y"], 1.0);
    assert_eq!(v["s"], -1.0);
    assert_eq!(v["a"], 0.25_f64.atan2(2.0));
    // y = 2x is clipped to [-1, 3], and the piecewise takes its first piece.
    assert_eq!(with_x(5.0)["y"], 3.0);
    assert_eq!(with_x(5.0)["s"], 10.0);
    assert_eq!(with_x(-5.0)["y"], -1.0);
}

#[test]
fn given_values_override_and_missing_inputs_fail() {
    let m = model();
    assert!(m.is_settable("x") && m.is_settable("k") && !m.is_settable("s") && !m.is_settable("q"));
    let given = BTreeMap::from([("x".to_owned(), 1.0), ("k".to_owned(), 0.5)]);
    assert_eq!(m.evaluate(&given).unwrap()["y"], 0.5);
    let err = m.evaluate(&BTreeMap::new()).unwrap_err().to_string();
    assert!(err.contains("input x not given"), "{err}");
}

#[test]
fn static_shots_are_read_with_tolerances_and_internal_values() {
    let root = parse_document(MODEL.as_bytes(), "m.dml").unwrap();
    let s = shots("m.dml", &root).unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].name, "one");
    assert_eq!(s[0].inputs["input"], 0.5);
    assert_eq!(s[0].outputs["clipped"], (1.0, 1e-6));
    assert_eq!(s[0].internal["y"], 1.0);
}
