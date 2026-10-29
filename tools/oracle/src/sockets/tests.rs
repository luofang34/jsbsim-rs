// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

const LOOPBACK: &str = "bind inet sent=127.0.0.1:5137 result=0 bound=127.0.0.1:5137\n\
                        bind inet sent=127.0.0.1:5139 result=-1 bound=-\n\
                        bind inet6 sent=::1:5139 result=0 bound=::1:5139\n";

#[test]
fn loopback_binds_of_every_declared_port_pass() {
    let binds = parse_report(LOOPBACK).unwrap();
    assert_eq!(binds.len(), 3);
    assert!(!binds[1].ok);
    assert_eq!(binds[1].bound, None);
    assert_eq!(problems(&binds, &[5137, 5139]), Vec::<String>::new());
}

#[test]
fn a_bind_to_every_interface_fails() {
    let binds = parse_report("bind inet sent=0.0.0.0:5139 result=0 bound=0.0.0.0:5139\n").unwrap();
    let found = problems(&binds, &[5139]);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("sent off loopback"), "{found:?}");
}

#[test]
fn an_unbound_port_or_another_family_fails() {
    assert_eq!(problems(&[], &[5137]), ["port 5137 was never bound"]);
    let binds = parse_report("bind other sent=?:0 result=0 bound=-\n").unwrap();
    assert_eq!(binds[0].sent, None);
    assert_eq!(problems(&binds, &[]).len(), 1);
}

#[test]
fn a_landing_off_loopback_or_on_an_undeclared_port_fails() {
    let binds = parse_report(
        "bind inet sent=127.0.0.1:5139 result=0 bound=10.0.0.2:5139\n\
         bind inet sent=127.0.0.1:6000 result=0 bound=127.0.0.1:6000\n",
    )
    .unwrap();
    let found = problems(&binds, &[5139]);
    assert_eq!(found.len(), 2, "{found:?}");
}

#[test]
fn malformed_lines_are_errors() {
    assert!(parse_report("bind inet sent=127.0.0.1:1\n").is_err());
    assert!(parse_report("connect inet sent=127.0.0.1:1 result=0 bound=-\n").is_err());
}
