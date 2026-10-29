// SPDX-License-Identifier: LGPL-2.1-or-later
//! `tripod_input`: the standing tripod of [`super::ground`] with two
//! `<input>` directives whose functions write properties, so that what
//! `FGInput` does besides reading its sockets is compared with C++:
//!
//! - a TCP input with no port (C++ binds an ephemeral port nobody connects
//!   to) declares a gain and copies, every frame, a counter advanced by the
//!   gain, a running sum of the altitude Propagate has just integrated and
//!   of the gear force of the frame before, and a post-function of both;
//! - a `QTJSBSIM` input on the 737's UDP port 5139 at 20 Hz counts its own
//!   frames, which `rate` spaces six frames apart at 120 Hz.
//!
//! `tripod_input.xml` disables `FGInput` for half a second, writes the
//! declared gain, and ground-trims the tripod as the `trim` golden does,
//! whose frames the inputs skip.

use crate::OracleError;

use super::ground::{ground_aircraft, north_pole};

/// The derived aircraft.
pub const AIRCRAFT: &str = "tripod_input";

const INPUTS: &str = r#"  <input>
    <property value="2.5">test/input/gain</property>
    <property value="0">test/input/counter</property>
    <property value="0">test/input/sum</property>
    <property value="0">test/input/post</property>
    <function copyto="test/input/counter">
      <sum><property>test/input/counter</property><property>test/input/gain</property></sum>
    </function>
    <function copyto="test/input/sum">
      <sum>
        <property>test/input/sum</property>
        <property>position/h-sl-ft</property>
        <property>forces/fbz-gear-lbs</property>
      </sum>
    </function>
    <function type="post" copyto="test/input/post">
      <product><property>test/input/counter</property><property>test/input/sum</property></product>
    </function>
  </input>
  <input type="QTJSBSIM" port="5139" rate="20">
    <property value="0">test/input/udp-counter</property>
    <function copyto="test/input/udp-counter">
      <sum><property>test/input/udp-counter</property><value>1</value></sum>
    </function>
  </input>
"#;

/// `tripod_input.xml` from the upstream `tests/tripod.xml`.
pub fn aircraft(tripod: &str) -> Result<String, OracleError> {
    let header = "  tripod_ground: tests/tripod.xml as JSBSim's tests/TestGndReactions.py";
    let ground = ground_aircraft(tripod)?.replacen(
        header,
        "  tripod_input: tripod_ground with two <input> directives whose functions\n  write properties.\n  tripod_ground: tests/tripod.xml as JSBSim's tests/TestGndReactions.py",
        1,
    );
    let close_at = ground
        .rfind("</fdm_config>")
        .ok_or_else(|| OracleError::Provenance("tests/tripod.xml: no </fdm_config>".to_owned()))?;
    Ok(format!(
        "{}{INPUTS}{}",
        &ground[..close_at],
        &ground[close_at..]
    ))
}

/// The script: the inputs disabled from 0.5 s to 1 s, the gain written at
/// 1.25 s and a ground trim at 1.5 s.
#[must_use]
pub fn script() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!--
  SPDX-License-Identifier: LGPL-2.1-or-later
  {AIRCRAFT}.xml: the tripod standing at the North pole with its <input>
  functions running, FGInput disabled from 0.5 s to 1 s, the input's
  declared gain written at 1.25 s, and a ground trim at 1.5 s, whose frames
  the inputs skip. Regenerate with `oracle derive`.
-->
<runscript name="{AIRCRAFT}">
  <use aircraft="{AIRCRAFT}" initialize="north_pole"/>
  <run start="0.0" end="3.0" dt="0.008333333333333333">
    <event name="Disable">
      <condition> simulation/sim-time-sec ge 0.5 </condition>
      <set name="simulation/models/FGInput/enabled" value="0"/>
    </event>
    <event name="Enable">
      <condition> simulation/sim-time-sec ge 1.0 </condition>
      <set name="simulation/models/FGInput/enabled" value="1"/>
    </event>
    <event name="Gain">
      <condition> simulation/sim-time-sec ge 1.25 </condition>
      <set name="test/input/gain" value="-1.0"/>
    </event>
    <event name="Trim">
      <condition> simulation/sim-time-sec ge 1.5 </condition>
      <set name="simulation/do_simple_trim" value="2"/>
    </event>
  </run>
</runscript>
"#
    )
}

/// Every derived file of this module, from the upstream `tests/tripod.xml`.
pub fn files(tripod: &str) -> Result<Vec<(String, String)>, OracleError> {
    Ok(vec![
        (
            format!("aircraft/{AIRCRAFT}/{AIRCRAFT}.xml"),
            aircraft(tripod)?,
        ),
        (format!("aircraft/{AIRCRAFT}/north_pole.xml"), north_pole()),
        (format!("scripts/{AIRCRAFT}.xml"), script()),
    ])
}
