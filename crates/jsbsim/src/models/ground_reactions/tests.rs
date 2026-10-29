// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use crate::fdm::Fdm;
use crate::math::{E_X, E_Z};
use crate::units::{DEGTORAD, RADTODEG};
use crate::{LoadOptions, MemoryResolver};

/// A structural point, a steerable retractable wheel and a castering
/// wheel, in that order.
const CONTACTS: &str = r#"
    <contact type="STRUCTURE" name="TAIL">
        <location unit="IN"> <x> 50 </x> <y> 0 </y> <z> 0 </z> </location>
    </contact>
    <contact type="BOGEY" name="NOSE">
        <location unit="IN"> <x> 0 </x> <y> 0 </y> <z> -24 </z> </location>
        <static_friction> 0.8 </static_friction>
        <dynamic_friction> 0.5 </dynamic_friction>
        <rolling_friction> 0.02 </rolling_friction>
        <spring_coeff unit="LBS/FT"> 1000 </spring_coeff>
        <damping_coeff type="SQUARE" unit="LBS/FT2/SEC2"> 30 </damping_coeff>
        <max_steer unit="DEG"> 10 </max_steer>
        <brake_group> LEFT </brake_group>
        <retractable> 1 </retractable>
    </contact>
    <contact type="BOGEY" name="CASTER">
        <location unit="IN"> <x> 0 </x> <y> 10 </y> <z> -24 </z> </location>
        <max_steer unit="DEG"> 360 </max_steer>
    </contact>
"#;

fn aircraft(contacts: &str) -> String {
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
    <ground_reactions>{contacts}</ground_reactions>
</fdm_config>"#
    )
}

fn load_with(options: LoadOptions, contacts: &str) -> Result<Fdm, Error> {
    let mut fdm = Fdm::with_options(options)?;
    // No <aerodynamics>: they would refuse to run.
    fdm.set_property("simulation/models/FGAerodynamics/enabled", 0.0)?;
    let mut resolver = MemoryResolver::new();
    resolver.insert("aircraft/t/t.xml", aircraft(contacts).into_bytes());
    fdm.load_model(&resolver, "aircraft", "t")?;
    Ok(fdm)
}

fn load() -> Fdm {
    load_with(LoadOptions::default(), CONTACTS).unwrap()
}

#[test]
fn constructor_reads_types_coefficients_and_steering() {
    let fdm = load();
    let c = &fdm.models.ground_reactions.contacts;
    assert_eq!(c.len(), 3);
    assert_eq!(c[0].kind, ContactType::Structure);
    // The executive's EmptyWeight input is not loaded yet, as in JSBSim.
    assert_eq!(c[0].strut.k_spring, 0.0);
    assert_eq!(c[0].friction.static_coeff, 1.0);
    assert_eq!(c[0].steering.kind, SteerType::Fixed);
    assert_eq!(c[0].brake_group, Some(BrakeGroup::None));
    assert_eq!(c[1].kind, ContactType::Bogey);
    assert_eq!(c[1].strut.k_spring, 1000.0);
    assert_eq!(c[1].strut.damp, DampType::Square);
    assert_eq!(c[1].strut.damp_rebound, DampType::Square);
    assert_eq!(c[1].strut.b_damp_rebound, 30.0);
    assert_eq!(c[1].friction.peak, 0.8);
    assert_eq!(c[1].steering.kind, SteerType::Steer);
    assert_eq!(c[1].brake_group, Some(BrakeGroup::Left));
    assert!(c[1].retractable);
    assert_eq!(c[2].steering.kind, SteerType::Caster);
    assert!(c[2].steering.castered);
}

#[test]
fn properties_use_indexed_names_and_index_zero_resolves_bare() {
    let mut fdm = load();
    let bare = fdm.node("contact/unit/WOW").unwrap();
    assert!(bare.is_some());
    assert_eq!(bare, fdm.node("contact/unit[0]/WOW").unwrap());
    for present in [
        "gear/unit[1]/pos-norm",
        "gear/unit[1]/wheel-speed-fps",
        "gear/unit[2]/castered",
        "gear/unit[2]/steering-angle-deg",
        "fcs/steer-pos-deg[1]",
        "fcs/steer-pos-deg[2]",
        "contact/unit[0]/static-friction-factor",
    ] {
        assert!(fdm.node(present).unwrap().is_some(), "{present}");
    }
    for absent in [
        "gear/unit[0]/WOW",
        "contact/unit[0]/pos-norm",
        "contact/unit[0]/slip-angle-deg",
        "gear/unit[1]/castered",
        "fcs/steer-pos-deg[0]",
    ] {
        assert!(fdm.node(absent).unwrap().is_none(), "{absent}");
    }
    assert_eq!(fdm.get_property("gear/num-units").unwrap(), 3.0);
    assert_eq!(
        fdm.get_property("gear/unit[1]/static_friction_coeff")
            .unwrap(),
        0.8
    );
}

#[test]
fn setters_behave_as_in_jsbsim() {
    let mut fdm = load();
    assert!(fdm.set_property("contact/unit/x-position", 12.0).unwrap());
    let c = &fdm.models.ground_reactions.contacts[0];
    assert_eq!((c.location[E_X], c.acting_location[E_X]), (12.0, 12.0));

    fdm.set_property("gear/unit[1]/static_friction_coeff", 0.6)
        .unwrap();
    assert_eq!(fdm.models.ground_reactions.contacts[1].friction.peak, 0.6);

    fdm.set_property("fcs/steer-cmd-norm", 0.5).unwrap();
    let expected = RADTODEG * (DEGTORAD * (0.5 * 10.0));
    assert_eq!(fdm.get_property("fcs/steer-pos-deg[1]").unwrap(), expected);
    // A castered wheel ignores steering commands.
    assert_eq!(fdm.get_property("fcs/steer-pos-deg[2]").unwrap(), 0.0);
    fdm.set_property("fcs/steer-pos-deg[2]", 3.0).unwrap();
    assert_eq!(fdm.get_property("fcs/steer-pos-deg[2]").unwrap(), 0.0);

    assert!(
        !fdm.set_property("gear/unit[1]/wheel-speed-fps", 1.0)
            .unwrap()
    );
    fdm.set_property("gear/unit[1]/WOW", 1.0).unwrap();
    assert_eq!(fdm.get_property("gear/wow").unwrap(), 1.0);
    fdm.set_property("contact/unit/WOW", 1.0).unwrap();
    fdm.set_property("gear/unit[1]/WOW", 0.0).unwrap();
    assert_eq!(fdm.get_property("gear/wow").unwrap(), 0.0, "BOGEY only");
}

#[test]
fn airborne_contacts_track_agl_without_weight_on_wheels() {
    let mut fdm = load();
    fdm.set_property("ic/h-agl-ft", 50.0).unwrap();
    fdm.run_ic().unwrap();
    let mut last = f64::INFINITY;
    let touchdown = loop {
        match fdm.run() {
            Ok(_) => {}
            Err(e) => break e,
        }
        let agl = fdm.get_property("contact/unit/AGL-ft").unwrap();
        let h = fdm.get_property("position/h-agl-ft").unwrap();
        assert!(agl <= last, "AGL follows the descent");
        assert!((agl - h).abs() < 1e-6, "{agl} vs {h}");
        last = agl;
        for p in [
            "contact/unit/WOW",
            "contact/unit/compression-ft",
            "gear/unit[1]/compression-velocity-fps",
            "gear/wow",
        ] {
            assert_eq!(fdm.get_property(p).unwrap(), 0.0, "{p}");
        }
    };
    // The nose wheel sits 2 ft below the CG, so it touches first.
    assert!(
        matches!(&touchdown, Error::Touchdown { contact, .. } if contact == "NOSE"),
        "{touchdown}"
    );
    assert!(last > 0.0 && last < 3.0, "{last}");
}

#[test]
fn steering_decays_while_the_gear_is_down_and_airborne() {
    let mut fdm = load();
    fdm.set_property("ic/h-agl-ft", 50.0).unwrap();
    fdm.run_ic().unwrap();
    fdm.set_property("fcs/steer-cmd-norm", 1.0).unwrap();
    let before = fdm.models.ground_reactions.contacts[1].steering.angle;
    fdm.run().unwrap();
    let after = fdm.models.ground_reactions.contacts[1].steering.angle;
    assert_eq!(after, before * ((1.0f64 - 0.8) / 0.2));
    assert!(after < before);
}

#[test]
fn unknown_brake_groups_and_tables_are_logged_or_refused_when_strict() {
    let odd = r#"
    <contact type="BOGEY" name="W">
        <location unit="IN"> <x> 0 </x> <y> 0 </y> <z> 0 </z> </location>
        <table name="SOMETHING"> <tableData> 0 1 </tableData> </table>
        <brake_group> SIDEWAYS </brake_group>
    </contact>"#;
    let fdm = load_with(LoadOptions::default(), odd).unwrap();
    let c = &fdm.models.ground_reactions.contacts[0];
    assert_eq!(c.brake_group, None);
    assert!(c.friction.cornering.is_none());
    let strict = load_with(LoadOptions { strict: true }, odd);
    assert!(matches!(strict.err(), Some(Error::Model { .. })));
}

#[test]
fn a_missing_location_is_refused() {
    let r = load_with(LoadOptions::default(), r#"<contact name="X"/>"#);
    assert!(matches!(r, Err(Error::Model { message }) if message.contains("No location")));
}

/// One contact at the CG, pressed `depth` ft into the ground by the
/// initial conditions; the run stops or goes on, the contact's frame is
/// computed either way.
fn pressed(contact: &str, depth: f64, setup: &[(&str, f64)]) -> Fdm {
    let mut fdm = load_with(LoadOptions::default(), contact).unwrap();
    fdm.set_property("ic/h-agl-ft", -depth).unwrap();
    for &(property, value) in setup {
        fdm.set_property(property, value).unwrap();
    }
    fdm.run_ic().ok();
    fdm
}

const STRUCTURE: &str = r#"
    <contact type="STRUCTURE" name="S">
        <location unit="IN"> <x> 0 </x> <y> 0 </y> <z> 0 </z> </location>
        <spring_coeff unit="LBS/FT"> 1000 </spring_coeff>
        <damping_coeff unit="LBS/FT/SEC"> 100 </damping_coeff>
    </contact>"#;

const WHEEL: &str = r#"
    <contact type="BOGEY" name="W">
        <location unit="IN"> <x> 0 </x> <y> 0 </y> <z> 0 </z> </location>
        <spring_coeff unit="LBS/FT"> 1000 </spring_coeff>
        <damping_coeff unit="LBS/FT/SEC"> 100 </damping_coeff>
        <static_friction> 0.8 </static_friction>
        <rolling_friction> 0.02 </rolling_friction>
        <brake_group> LEFT </brake_group>
    </contact>"#;

#[test]
fn a_pressed_structure_pushes_back_along_the_ground_normal() {
    let mut fdm = pressed(STRUCTURE, 0.1, &[]);
    let c = &fdm.models.ground_reactions.contacts[0];
    assert!(c.state.wow);
    assert!((c.state.compress_length - 0.1).abs() < 1e-6);
    // RunIC runs with dt = 0, where no compression speed is computed.
    assert_eq!(c.state.compress_speed, 0.0);
    assert_eq!(c.state.strut_force, -c.state.compress_length * 1000.0);
    assert_eq!(c.force.normal_force[E_Z], -c.state.strut_force);
    let forces = fdm.models.ground_reactions.forces;
    assert!((forces[E_Z] + 100.0).abs() < 1e-4, "{forces:?}");
    // Static friction holds the contact: roll and side constraints.
    let registered = &fdm.models.ground_reactions.multipliers;
    assert_eq!(
        registered,
        &[(0, FrictionType::Roll), (0, FrictionType::Side)]
    );
    let m = &c.force.multipliers;
    assert_eq!(m.roll.max, (1.0 * 1.0 * c.force.normal_force[E_Z]).abs());
    assert_eq!(m.roll.min, -m.roll.max);
    assert!(fdm.get_property("contact/unit/WOW").unwrap() != 0.0);
}

#[test]
fn a_strut_function_replaces_spring_and_damping() {
    let strut = r#"
    <contact type="BOGEY" name="W">
        <location unit="IN"> <x> 0 </x> <y> 0 </y> <z> 0 </z> </location>
        <strut_force>
            <function>
                <product>
                    <property>gear/unit/compression-ft</property>
                    <value>-2000</value>
                </product>
            </function>
        </strut_force>
    </contact>"#;
    let fdm = pressed(strut, 0.1, &[]);
    let c = &fdm.models.ground_reactions.contacts[0];
    assert!(c.state.compress_length > 0.0);
    assert_eq!(c.state.strut_force, c.state.compress_length * -2000.0);
}

#[test]
fn a_maximum_force_below_the_strut_force_replaces_it() {
    // JSBSim compares the (never positive) strut force with the maximum
    // as is, so only a maximum below it, past -100 lbs here, applies.
    let fdm = pressed(STRUCTURE, 0.1, &[("ground/maximum-force-lbs", -50.0)]);
    let c = &fdm.models.ground_reactions.contacts[0];
    assert_eq!(c.state.strut_force, -c.state.compress_length * 1000.0);
    let fdm = pressed(STRUCTURE, 0.1, &[("ground/maximum-force-lbs", -150.0)]);
    let c = &fdm.models.ground_reactions.contacts[0];
    assert_eq!(c.state.strut_force, -150.0);
    assert_eq!(c.state.compress_length, 150.0 / 1000.0);
}

#[test]
fn brakes_add_their_share_of_static_friction() {
    let fdm = pressed(WHEEL, 0.1, &[("fcs/left-brake-cmd-norm", 0.5)]);
    let c = &fdm.models.ground_reactions.contacts[0];
    assert_eq!(c.force.brake_coeff, 1.0 * 0.02 + 0.5 * 1.0 * (0.8 - 0.02));
    let normal = c.force.normal_force[E_Z];
    assert_eq!(
        c.force.multipliers.roll.max,
        (c.force.brake_coeff * normal).abs()
    );
    let released = pressed(WHEEL, 0.1, &[("fcs/right-brake-cmd-norm", 0.5)]);
    let c = &released.models.ground_reactions.contacts[0];
    assert_eq!(c.force.brake_coeff, 1.0 * 0.02);
}

#[test]
fn a_still_wheel_keeps_zero_slip_and_side_friction() {
    let fdm = pressed(WHEEL, 0.1, &[]);
    let c = &fdm.models.ground_reactions.contacts[0];
    assert_eq!(c.state.wheel_slip, 0.0);
    assert_eq!(c.state.side_coeff, 0.0);
    assert_eq!(c.force.multipliers.side.max, 0.0);
}

#[test]
fn wheels_sink_into_liquid_ground_without_weight_on_them() {
    let mut fdm = load_with(LoadOptions::default(), WHEEL).unwrap();
    fdm.set_property("ic/h-agl-ft", -0.1).unwrap();
    fdm.set_property("ground/solid", 0.0).unwrap();
    fdm.run_ic().unwrap();
    let c = &fdm.models.ground_reactions.contacts[0];
    assert!(!c.state.wow);
    assert_eq!(c.state.compress_length, 0.0);
    assert_eq!(
        fdm.models.ground_reactions.forces,
        crate::math::ColumnVector3::zero()
    );
}

#[test]
fn bumps_are_evaluated_at_the_origin_jsbsim_never_moves() {
    let mut surface = Surface::default();
    assert_eq!(surface.bump_height(), 0.0);
    surface.bumpiness = 0.5;
    assert_eq!(surface.bump_height(), 0.0);
    surface.pos = [2.5, 7.5, 0.0];
    let (x, y) = (
        0.25 * 2.0 * std::f64::consts::PI,
        0.75 * 2.0 * std::f64::consts::PI,
    );
    let s = libm::sin;
    let h = s(x)
        + s(7.0 * x)
        + s(8.0 * x)
        + s(13.0 * x)
        + (s(2.0 * y) + s(5.0 * y) + s(9.0 * y * x) + s(17.0 * y));
    assert_eq!(surface.bump_height(), h * (1.0 / 8.) * 0.5 * 0.4);

    let mut fdm = load_with(LoadOptions::default(), WHEEL).unwrap();
    fdm.set_property("ic/h-agl-ft", 10.0).unwrap();
    fdm.set_property("ground/bumpiness", 0.5).unwrap();
    fdm.run_ic().unwrap();
    fdm.run().unwrap();
}
