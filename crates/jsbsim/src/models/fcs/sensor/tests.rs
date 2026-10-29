// SPDX-License-Identifier: LGPL-2.1-or-later
use crate::models::fcs::tests::{load_error, load_system};
use crate::random::RandomNumberGenerator;
use crate::{Error, LoadOptions};

fn sensor(body: &str) -> String {
    format!(
        r#"<system>
  <channel name="c">
    <sensor name="test/s">
      <input>test/in</input>
      {body}
    </sensor>
  </channel>
</system>"#
    )
}

#[test]
fn absolute_gaussian_noise_draws_from_the_executive() {
    let system = sensor(r#"<noise variation="ABSOLUTE" distribution="GAUSSIAN"> 0.1 </noise>"#);
    let mut fdm = load_system(&system, LoadOptions::default()).unwrap();
    fdm.set_property("simulation/randomseed", 3.0).unwrap();
    fdm.set_property("test/in", 2.5).unwrap();
    let expected = RandomNumberGenerator::new(3);
    for _ in 0..4 {
        fdm.run().unwrap();
        let value = 2.5 + 0.1 * expected.normal();
        assert_eq!(
            fdm.get_property("test/s").unwrap().to_bits(),
            value.to_bits()
        );
    }
}

#[test]
fn percent_uniform_noise_draws_from_the_sensor_once_seeded() {
    let system = sensor(r#"<noise variation="PERCENT" distribution="UNIFORM"> 0.2 </noise>"#);
    let mut fdm = load_system(&system, LoadOptions::default()).unwrap();
    fdm.set_property("test/s/randomseed", 5.0).unwrap();
    fdm.set_property("test/in", 2.5).unwrap();
    let expected = RandomNumberGenerator::new(5);
    for exec_seed in [1.0, 2.0, 3.0] {
        // The executive's seed no longer reaches the sensor.
        fdm.set_property("simulation/randomseed", exec_seed)
            .unwrap();
        fdm.run().unwrap();
        let value = 2.5 * (1.0 + 0.2 * expected.uniform());
        assert_eq!(
            fdm.get_property("test/s").unwrap().to_bits(),
            value.to_bits()
        );
    }
}

#[test]
fn a_zero_noise_draws_nothing_and_loads() {
    let system = sensor(r#"<noise variation="ABSOLUTE" distribution="GAUSSIAN"> 0 </noise>"#);
    let mut fdm = load_system(&system, LoadOptions::default()).unwrap();
    fdm.set_property("test/in", 2.5).unwrap();
    fdm.run().unwrap();
    assert_eq!(fdm.get_property("test/s").unwrap(), 2.5);
}

#[test]
fn an_unknown_noise_type_fails_in_strict_mode_only() {
    let system = sensor("<noise> 0 </noise>");
    assert!(load_system(&system, LoadOptions::default()).is_ok());
    let strict = LoadOptions { strict: true };
    assert!(matches!(load_error(&system, strict), Error::Model { .. }));
}

#[test]
fn quantization_clamps_and_counts_steps() {
    let system = sensor(
        r#"<quantization name="q"> <bits> 2 </bits> <min> 0 </min> <max> 4 </max> </quantization>"#,
    );
    let mut fdm = load_system(&system, LoadOptions::default()).unwrap();
    for (input, output, steps) in [(2.7, 2.0, 2.0), (9.0, 4.0, 4.0), (-1.0, 0.0, 0.0)] {
        fdm.set_property("test/in", input).unwrap();
        fdm.run().unwrap();
        assert_eq!(fdm.get_property("test/s").unwrap(), output);
        assert_eq!(fdm.get_property("fcs/q").unwrap(), steps);
    }
}

#[test]
fn failures_override_and_hold_the_output() {
    let mut fdm = load_system(&sensor("<bias> 1 </bias>"), LoadOptions::default()).unwrap();
    fdm.set_property("test/in", 1.0).unwrap();
    fdm.run().unwrap();
    assert_eq!(fdm.get_property("test/s").unwrap(), 2.0);
    fdm.set_property("test/s/malfunction/fail_stuck", 1.0)
        .unwrap();
    fdm.set_property("test/in", 5.0).unwrap();
    fdm.run().unwrap();
    assert_eq!(fdm.get_property("test/s").unwrap(), 2.0);
    fdm.set_property("test/s/malfunction/fail_stuck", 0.0)
        .unwrap();
    fdm.set_property("test/s/malfunction/fail_low", 0.5)
        .unwrap();
    fdm.run().unwrap();
    assert_eq!(fdm.get_property("test/s").unwrap(), f64::NEG_INFINITY);
    assert_eq!(
        fdm.get_property("test/s/malfunction/fail_low").unwrap(),
        1.0
    );
}

#[test]
fn out_of_range_bits_are_refused() {
    let system = sensor("<quantization> <bits> 31 </bits> </quantization>");
    assert!(matches!(
        load_error(&system, LoadOptions::default()),
        Error::Model { .. }
    ));
}

#[test]
fn the_random_seed_reads_the_executive_until_written() {
    let mut fdm = load_system(&sensor(""), LoadOptions::default()).unwrap();
    fdm.set_property("simulation/randomseed", 7.0).unwrap();
    assert_eq!(fdm.get_property("test/s/randomseed").unwrap(), 7.0);
    fdm.set_property("test/s/randomseed", 5.9).unwrap();
    assert_eq!(fdm.get_property("test/s/randomseed").unwrap(), 5.0);
}
