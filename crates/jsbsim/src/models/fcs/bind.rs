// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGFCS.cpp (bind).
//
//! Properties of `FGFCS`, in JSBSim's binding order.

use super::{Fcs, Surface};
use crate::Error;
use crate::fdm::{Binder, Kind, Models};

fn commands(b: &mut Binder<'_>) -> Result<(), Error> {
    macro_rules! cmd {
        ($path:literal, $field:ident) => {
            b.property(
                $path,
                Kind::Double,
                |m| m.fcs.$field,
                |m, v| m.fcs.$field = v,
            )?;
        };
    }
    cmd!("fcs/aileron-cmd-norm", da_cmd);
    cmd!("fcs/elevator-cmd-norm", de_cmd);
    cmd!("fcs/rudder-cmd-norm", dr_cmd);
    cmd!("fcs/flap-cmd-norm", df_cmd);
    cmd!("fcs/speedbrake-cmd-norm", dsb_cmd);
    cmd!("fcs/spoiler-cmd-norm", dsp_cmd);
    cmd!("fcs/pitch-trim-cmd-norm", ptrim_cmd);
    cmd!("fcs/roll-trim-cmd-norm", rtrim_cmd);
    cmd!("fcs/yaw-trim-cmd-norm", ytrim_cmd);
    Ok(())
}

fn surface(
    b: &mut Binder<'_>,
    name: &str,
    get: fn(&Models) -> &Surface,
    set: fn(&mut Models) -> &mut Surface,
) -> Result<(), Error> {
    for (suffix, form) in [("rad", 0), ("deg", 1), ("norm", 2)] {
        b.property(
            &format!("fcs/{name}-pos-{suffix}"),
            Kind::Double,
            move |m| get(m).0[form],
            move |m, v| set(m).set(form, v),
        )?;
    }
    b.getter(&format!("fcs/mag-{name}-pos-rad"), move |m| get(m).0[3])?;
    Ok(())
}

/// `FGFCS::bind`.
pub fn bind(b: &mut Binder<'_>) -> Result<(), Error> {
    commands(b)?;
    surface(b, "left-aileron", |m| &m.fcs.dal, |m| &mut m.fcs.dal)?;
    surface(b, "right-aileron", |m| &m.fcs.dar, |m| &mut m.fcs.dar)?;
    surface(b, "elevator", |m| &m.fcs.de, |m| &mut m.fcs.de)?;
    surface(b, "rudder", |m| &m.fcs.dr, |m| &mut m.fcs.dr)?;
    surface(b, "flap", |m| &m.fcs.df, |m| &mut m.fcs.df)?;
    surface(b, "speedbrake", |m| &m.fcs.dsb, |m| &mut m.fcs.dsb)?;
    surface(b, "spoiler", |m| &m.fcs.dsp, |m| &mut m.fcs.dsp)?;
    let d = Kind::Double;
    b.property(
        "gear/gear-pos-norm",
        d,
        |m| m.fcs.gear_pos,
        |m, v| m.fcs.gear_pos = v,
    )?;
    b.property(
        "gear/gear-cmd-norm",
        d,
        |m| m.fcs.gear_cmd,
        |m, v| m.fcs.gear_cmd = v,
    )?;
    for (name, group) in [("left", 1), ("right", 2), ("center", 3)] {
        b.property(
            &format!("fcs/{name}-brake-cmd-norm"),
            d,
            move |m| m.fcs.brake_pos[group],
            move |m, v| m.fcs.brake_pos[group] = v,
        )?;
    }
    b.property(
        "gear/tailhook-pos-norm",
        d,
        |m| m.fcs.tailhook_pos,
        |m, v| m.fcs.tailhook_pos = v,
    )?;
    b.property(
        "fcs/wing-fold-pos-norm",
        d,
        |m| m.fcs.wing_fold_pos,
        |m, v| m.fcs.wing_fold_pos = v,
    )?;
    b.getter("simulation/channel-dt", |m| Fcs::channel_delta_t(&m.fcs))?;
    Ok(())
}
