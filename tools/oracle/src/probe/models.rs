// SPDX-License-Identifier: LGPL-2.1-or-later
//! Model-loading probe; twin of `tools/oracle/cpp/probe_models.cpp`. An
//! aircraft that needs a construct the port does not cover yet prints
//! `unsupported <construct> (<where>)` instead of its properties.

use jsbsim::FileResolver;
use jsbsim::c_numeric::format_g;
use jsbsim::fdm::Fdm;
use jsbsim::properties::{NodeId, ValueType};

use super::ProbeContext;

/// Property subtrees of the models the port covers; keep in step with
/// `kPrefixes` in the C++ probe.
pub const PREFIXES: [&str; 17] = [
    "inertia/",
    "metrics/",
    "position/",
    "velocities/",
    "attitude/",
    "accelerations/",
    "forces/",
    "moments/",
    "aero/",
    "atmosphere/",
    "external_reactions/",
    "fcs/",
    "systems/",
    "simulation/",
    "contact/",
    "gear/",
    "propulsion/",
];

/// Contact properties tied to `FGLGear` members its constructor leaves
/// uninitialized until the first force calculation (`AGL`, FGLGear.h:369,
/// first set at FGLGear.cpp:304; `FCoeff`, FGLGear.h:350), so before any
/// frame C++ reads whatever memory held. Keep in step with
/// `kUninitialized` in the C++ probe.
pub const UNINITIALIZED: [&str; 2] = ["/AGL-ft", "/side_friction_coeff"];

/// What the walk strips from fully qualified names: the instance root,
/// as `GetRelativeName(child, "/fdm/jsbsim/")` in the C++ probe. Nodes an
/// absolute path made outside the instance are not walked.
const INSTANCE_PREFIX: &str = "/fdm/jsbsim/";

fn walk(fdm: &mut Fdm, node: NodeId, lines: &mut Vec<String>) -> Result<(), jsbsim::Error> {
    let children = fdm.tree().children(node).to_vec();
    for child in children {
        if !fdm.tree().children(child).is_empty() {
            walk(fdm, child, lines)?;
            continue;
        }
        let path = fdm.tree().relative_name(child, INSTANCE_PREFIX);
        if !PREFIXES.iter().any(|p| path.starts_with(p))
            || UNINITIALIZED.iter().any(|s| path.ends_with(s))
        {
            continue;
        }
        match fdm.tree().value_type(child) {
            ValueType::Bool
            | ValueType::Int
            | ValueType::Long
            | ValueType::Float
            | ValueType::Double => {
                let v = fdm.read_node(child)?;
                lines.push(format!("{path} = {}", format_g(v, 17)));
            }
            ValueType::String | ValueType::Unspecified => {
                lines.push(format!("{path} = \"{}\"", fdm.tree().get_string(child)));
            }
            ValueType::None => {}
        }
    }
    Ok(())
}

fn load(name: &str, context: &ProbeContext) -> String {
    let root = context.jsbsim_root.clone().unwrap_or_default();
    let resolver = FileResolver::new(&root);
    let result = Fdm::new().and_then(|mut fdm| {
        fdm.load_model(&resolver, "aircraft", name)?;
        let mut lines = Vec::new();
        let top = fdm.tree().root();
        walk(&mut fdm, top, &mut lines)?;
        Ok(lines)
    });
    match result {
        Ok(mut lines) => {
            lines.sort();
            let mut out = " ok".to_owned();
            for l in lines {
                out.push_str("\n  ");
                out.push_str(&l);
            }
            out
        }
        Err(jsbsim::Error::Unsupported { construct, path }) => {
            format!(" unsupported {construct} ({path})")
        }
        Err(e) => {
            tracing::debug!("{name}: load-error: {e}");
            " load-error".to_owned()
        }
    }
}

/// Runs the probe on `input`.
#[must_use]
pub fn run(input: &str, context: &ProbeContext) -> String {
    let mut out = String::new();
    for line in input.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace();
        let result = match (words.next(), words.next()) {
            (Some("model"), Some(name)) => load(name, context),
            _ => " unknown".to_owned(),
        };
        out.push_str(&format!("{line} =>{result}\n"));
    }
    out
}
