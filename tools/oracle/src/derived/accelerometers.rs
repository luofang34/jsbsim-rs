// SPDX-License-Identifier: LGPL-2.1-or-later
//! The ball of JSBSim's `tests/TestAccelerometer.py` with
//! `systems/accelerometers.xml` included, as its
//! `AddAccelerometersToAircraft` derives it (a `<system
//! file="accelerometers">` element appended to the aircraft), and scripts
//! flying it the way the test's ball cases do:
//!
//! - `ball_orbit_accelerometers.xml` is `scripts/ball_orbit.xml` on the
//!   derived ball at `dt` 0.1, as `testOrbit` edits it, with
//!   `fcs/accelerometer/on` set before `RunIC` as the test sets it;
//! - `ball_spin_accelerometers.xml` loads `reset00` and sets what
//!   `testSpinningBodyOnOrbit` sets before `RunIC`: the CG offset by a
//!   50 lb point mass, a 90 degree roll and an inertial yaw rate of
//!   1 rad/s.

use crate::OracleError;

/// The derived aircraft's name.
pub const AIRCRAFT: &str = "ball_accelerometers";

/// `omega` of the test, the Earth rotation rate in rad/s.
const OMEGA: f64 = 0.00007292115;

fn fail(source: &str, message: &str) -> OracleError {
    OracleError::Provenance(format!("{source}: {message}"))
}

/// The derived aircraft text from the upstream `aircraft/ball/ball.xml`.
pub fn aircraft(text: &str) -> Result<String, OracleError> {
    let source = "aircraft/ball/ball.xml";
    let header_at = text
        .find("<fdm_config")
        .ok_or_else(|| fail(source, "no <fdm_config>"))?;
    let close_at = text
        .rfind("</fdm_config>")
        .ok_or_else(|| fail(source, "no </fdm_config>"))?;
    let mut out = String::with_capacity(text.len() + 512);
    out.push_str(&text[..header_at]);
    out.push_str(&format!(
        "<!--\n  SPDX-License-Identifier: LGPL-2.1-or-later\n  {AIRCRAFT}: {source} with <system file=\"accelerometers\"/>\n  appended, as JSBSim's tests/TestAccelerometer.py\n  AddAccelerometersToAircraft derives it. Regenerate with `oracle derive`.\n-->\n"
    ));
    out.push_str(&text[header_at..close_at]);
    out.push_str("  <system file=\"accelerometers\"/>\n");
    out.push_str(&text[close_at..]);
    Ok(out)
}

/// `ball_orbit_accelerometers.xml` from the upstream
/// `scripts/ball_orbit.xml`.
pub fn orbit_script(text: &str) -> Result<String, OracleError> {
    let source = "scripts/ball_orbit.xml";
    let mut out = text.to_owned();
    let mut replace = |from: &str, to: &str| {
        let at = out
            .find(from)
            .ok_or_else(|| fail(source, &format!("no {from}")))?;
        out.replace_range(at..at + from.len(), to);
        Ok::<(), OracleError>(())
    };
    replace(
        "<use aircraft=\"ball\"",
        &format!("<use aircraft=\"{AIRCRAFT}\""),
    )?;
    replace("dt=\"0.005\">", "dt=\"0.1\">")?;
    replace(
        "    <property value=\"3\"> simulation/integrator/rate/rotational </property>\n",
        "    <property value=\"1\"> fcs/accelerometer/on </property>\n    <property value=\"3\"> simulation/integrator/rate/rotational </property>\n",
    )?;
    let at = out
        .find("<runscript")
        .ok_or_else(|| fail(source, "no <runscript>"))?;
    out.insert_str(
        at,
        "<!--\n  SPDX-License-Identifier: LGPL-2.1-or-later\n  ball_orbit_accelerometers.xml: scripts/ball_orbit.xml on\n  ball_accelerometers at dt 0.1 with fcs/accelerometer/on set, as\n  JSBSim's tests/TestAccelerometer.py testOrbit runs it. Regenerate\n  with `oracle derive`.\n-->\n",
    );
    Ok(out)
}

/// `ball_spin_accelerometers.xml`: `testSpinningBodyOnOrbit`'s settings
/// as a script, run on for 20 s.
#[must_use]
pub fn spin_script() -> String {
    let r = format!("{:?}", 1.0 - OMEGA);
    let phi = format!("{:?}", 0.5 * std::f64::consts::PI);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!--
  SPDX-License-Identifier: LGPL-2.1-or-later
  ball_spin_accelerometers.xml: JSBSim's tests/TestAccelerometer.py
  testSpinningBodyOnOrbit as a script: ball_accelerometers from reset00
  with its CG offset along Y by a 50 lb point mass, rolled 90 degrees so
  that it spins about Z, at an inertial yaw rate of 1 rad/s (r = 1 -
  omega). Regenerate with `oracle derive`.
-->
<runscript name="spinning ball accelerometers">
  <use aircraft="{AIRCRAFT}" initialize="reset00"/>
  <run start="0.0" end="20" dt="0.01">
    <property value="1"> fcs/accelerometer/on </property>
    <property value="50.0"> inertia/pointmass-weight-lbs[1] </property>
    <property value="{phi}"> ic/phi-rad </property>
    <property value="0.0"> ic/p-rad_sec </property>
    <property value="0.0"> ic/q-rad_sec </property>
    <property value="{r}"> ic/r-rad_sec </property>
  </run>
</runscript>
"#
    )
}
