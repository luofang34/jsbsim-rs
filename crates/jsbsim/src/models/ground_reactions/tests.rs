// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;
use crate::fdm::Fdm;
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
    assert_eq!((c.location[1], c.acting_location[1]), (12.0, 12.0));

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
