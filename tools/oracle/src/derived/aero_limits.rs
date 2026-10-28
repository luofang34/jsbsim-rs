// SPDX-License-Identifier: LGPL-2.1-or-later
//! The mk82 with the `<aerodynamics>` elements corpus aircraft use that
//! the mk82 lacks: `<alphalimits>` in degrees (as Concorde writes them),
//! `<hysteresis_limits>` in radians (as c172p does), an
//! `<aero_ref_pt_shift_x>` table (as Concorde and f22 have), and functions
//! that feed `systems/stall-warn-norm` and `aero/stall-hyst-norm` back
//! into the forces, one applied at the CG. The aircraft that carry them
//! stop at `<propulsion>`, so this one exercises FGAerodynamics' stall,
//! hysteresis and reference-point paths. Its script drops the mk82 from
//! `reset00`, whose angle of attack swings through the hysteresis band,
//! and moves `aero/alpha-max-rad` mid-run through its setter.

use crate::OracleError;

/// The derived aircraft's name.
pub const AIRCRAFT: &str = "mk82_aero_limits";

const SOURCE: &str = "aircraft/mk82/mk82.xml";

const LIMITS: &str = r#"
        <alphalimits unit="DEG">
            <min>-11.5</min>
            <max>8.6</max>
        </alphalimits>
        <hysteresis_limits unit="RAD">
            <min>-0.1</min>
            <max>0.1</max>
        </hysteresis_limits>
        <aero_ref_pt_shift_x>
            <function name="aero/function/rp-shift">
                <table>
                    <independentVar>aero/alpha-rad</independentVar>
                    <tableData>
                        -0.4  -0.05
                         0.0   0.0
                         0.4   0.08
                    </tableData>
                </table>
            </function>
        </aero_ref_pt_shift_x>
"#;

const DRAG: &str = r#"            <function name="aero/coefficient/CD-stall-warn">
                <product>
                    <property>aero/qbar-psf</property>
                    <property>metrics/Sw-sqft</property>
                    <property>systems/stall-warn-norm</property>
                    <value>0.01</value>
                </product>
            </function>
"#;

const LIFT_AT_CG: &str = r#"            <function name="aero/coefficient/CL-hyst" apply_at_cg="true">
                <product>
                    <property>aero/qbar-psf</property>
                    <property>metrics/Sw-sqft</property>
                    <property>aero/stall-hyst-norm</property>
                    <value>0.05</value>
                </product>
            </function>
"#;

fn fail(message: &str) -> OracleError {
    OracleError::Provenance(format!("{SOURCE}: {message}"))
}

/// Inserts `text` just before the `</axis>` that closes axis `name`.
fn append_to_axis(out: &mut String, name: &str, text: &str) -> Result<(), OracleError> {
    let start = out
        .find(&format!("<axis name=\"{name}\">"))
        .ok_or_else(|| fail(&format!("no {name} axis")))?;
    let close = out
        .get(start..)
        .and_then(|rest| rest.find("        </axis>"))
        .ok_or_else(|| fail(&format!("unterminated {name} axis")))?;
    out.insert_str(start + close, text);
    Ok(())
}

/// The derived aircraft text from the upstream `aircraft/mk82/mk82.xml`.
pub fn aircraft(text: &str) -> Result<String, OracleError> {
    let header_at = text
        .find("<fdm_config")
        .ok_or_else(|| fail("no <fdm_config>"))?;
    let aero = "<aerodynamics>";
    let aero_at = text.find(aero).ok_or_else(|| fail("no <aerodynamics>"))? + aero.len();
    let mut out = String::with_capacity(text.len() + 2048);
    out.push_str(text.get(..header_at).unwrap_or_default());
    out.push_str(&format!(
        "<!--\n  SPDX-License-Identifier: LGPL-2.1-or-later\n  {AIRCRAFT}: {SOURCE} with stall and hysteresis limits, a\n  reference-point shift and stall-dependent DRAG and LIFT (at the CG)\n  functions added to <aerodynamics>. Regenerate with `oracle derive`.\n-->\n"
    ));
    out.push_str(text.get(header_at..aero_at).unwrap_or_default());
    out.push_str(LIMITS);
    out.push_str(text.get(aero_at..).unwrap_or_default());
    append_to_axis(&mut out, "DRAG", DRAG)?;
    append_to_axis(&mut out, "LIFT", LIFT_AT_CG)?;
    Ok(out)
}

/// The derived aircraft, its `reset00` (the mk82's, unchanged) and its
/// script, by path under `oracle/derived`.
pub fn files(mk82: &str, reset: &str) -> Result<Vec<(String, String)>, OracleError> {
    Ok(vec![
        (
            format!("aircraft/{AIRCRAFT}/{AIRCRAFT}.xml"),
            aircraft(mk82)?,
        ),
        (format!("aircraft/{AIRCRAFT}/reset00.xml"), reset.to_owned()),
        (format!("scripts/{AIRCRAFT}_script.xml"), script()),
    ])
}

/// The script flying the derived aircraft.
#[must_use]
pub fn script() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!--
  SPDX-License-Identifier: LGPL-2.1-or-later
  {AIRCRAFT}_script.xml: scripts/mk82_script.xml on {AIRCRAFT} for
  10 s, with aero/alpha-max-rad lowered at 2 s. Regenerate with
  `oracle derive`.
-->
<runscript name="mk82 aero limits">
  <use aircraft="{AIRCRAFT}" initialize="reset00"/>
  <run start="0.00" end="10.00" dt="0.020">
    <property value="3">simulation/integrator/position/rotational</property>
    <event name="Lower the stall limit">
      <condition>simulation/sim-time-sec ge 2.0</condition>
      <set name="aero/alpha-max-rad" value="0.05"/>
    </event>
  </run>
</runscript>
"#
    )
}
