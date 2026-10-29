// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

fn frame(values: &[(&str, u64)], state: u8) -> Frame {
    Frame {
        values: values.iter().map(|(p, v)| ((*p).to_owned(), *v)).collect(),
        state: [state; 32],
    }
}

#[test]
fn compare_names_the_first_difference() {
    let a = [frame(&[("x", 1), ("y", 2)], 0), frame(&[("x", 3)], 0)];
    assert_eq!(compare(&a, &a), None);
    let b = [frame(&[("x", 1), ("y", 5)], 0), frame(&[("x", 3)], 0)];
    assert_eq!(
        compare(&a, &b),
        Some(Mismatch::Value {
            frame: 0,
            path: "y".to_owned(),
            a: 2,
            b: 5
        })
    );
    let c = [frame(&[("x", 1), ("y", 2)], 0), frame(&[("x", 3)], 9)];
    assert_eq!(compare(&a, &c), Some(Mismatch::State { frame: 1 }));
    assert_eq!(compare(&a, &a[..1]), Some(Mismatch::Length { a: 2, b: 1 }));
    let d = [frame(&[("x", 1), ("z", 2)], 0), frame(&[("x", 3)], 0)];
    assert_eq!(compare(&a, &d), Some(Mismatch::Nodes { frame: 0 }));
}
