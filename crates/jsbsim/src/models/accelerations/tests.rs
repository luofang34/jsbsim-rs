// SPDX-License-Identifier: LGPL-2.1-or-later
use super::*;

/// A 2-slug body with unit inertia sliding along x at 1 ft/s, held by one
/// friction constraint along x through the CG, bounded by `limit`.
#[cfg(test)]
fn sliding(limit: f64) -> Accelerations {
    let identity = Matrix33::new(1., 0., 0., 0., 1., 0., 0., 0., 1.);
    let mut a = Accelerations::default();
    a.base.rate = 1;
    let i = &mut a.input;
    (i.j, i.jinv, i.ti2b, i.tb2i, i.tec2b, i.tec2i) =
        (identity, identity, identity, identity, identity, identity);
    i.mass = 2.0;
    i.delta_t = 0.5;
    i.uvw = ColumnVector3::new(1., 0., 0.);
    i.multipliers.push(LagrangeMultiplier {
        force_jacobian: ColumnVector3::new(1., 0., 0.),
        min: -limit,
        max: limit,
        ..LagrangeMultiplier::default()
    });
    a
}

#[test]
fn friction_stops_the_relative_motion_within_the_step() {
    let mut a = sliding(100.0);
    a.run(false, false, false);
    let m = a.input.multipliers.first().copied().unwrap();
    // The force that zeroes 1 ft/s over 0.5 s on 2 slugs.
    assert_eq!(m.value, -4.0);
    assert_eq!(a.friction_forces, ColumnVector3::new(-4., 0., 0.));
    assert_eq!(a.uvwdot, ColumnVector3::new(-2., 0., 0.));
    assert_eq!(a.body_accel, ColumnVector3::new(-2., 0., 0.));
}

#[test]
fn friction_is_bounded_by_the_constraint() {
    let mut a = sliding(1.0);
    a.run(false, false, false);
    assert_eq!(a.input.multipliers.first().unwrap().value, -1.0);
    assert_eq!(a.uvwdot, ColumnVector3::new(-0.5, 0., 0.));
}

#[test]
fn held_down_vehicles_get_no_friction() {
    let mut a = sliding(100.0);
    a.run(false, true, false);
    assert_eq!(a.friction_forces, ColumnVector3::zero());
    assert_eq!(a.input.multipliers.first().unwrap().value, 0.0);
}
