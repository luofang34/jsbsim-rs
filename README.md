# jsbsim-rs

An **unofficial** pure-Rust port of [JSBSim](https://github.com/JSBSim-Team/jsbsim),
the open-source flight dynamics model. It is not affiliated with or endorsed by
the JSBSim project.

The `jsbsim` crate has no C++ dependency and builds for native targets and for
`wasm32-unknown-unknown`.

## Licence

LGPL-2.1-or-later, the licence of JSBSim; see [LICENSE](LICENSE). Source files
derived from JSBSim keep the copyright lines of the C++ files they translate and
name those files.

## Build and test

The toolchain is pinned in `rust-toolchain.toml`.

```sh
cargo build
cargo test --workspace --all-targets
scripts/ci.sh            # every local CI gate, in order
```

`scripts/ci.sh` runs `cargo fmt --check`, Clippy with `-D warnings`, the tests,
`cargo doc` with `-D missing_docs -D broken_intra_doc_links`, a release build,
the `wasm32-unknown-unknown` build of `jsbsim`, and the structural checks in
`scripts/check-*.sh`.

## Oracle

`tools/oracle` compares this port against C++ JSBSim. Its inputs and outputs
live in `oracle/`:

- `oracle/goldens/<case>/manifest.toml`: the script to run, the sampling rate
  and the properties to log (`[case]`), plus the upstream commit, time step and
  row count of the committed reference (`[provenance]`).
- `oracle/goldens/<case>/reference.csv`: C++ output, sampled at full double
  precision by `tools/oracle/cpp/oracle_driver.cpp`.
- `oracle/tolerances.toml`: per case and property, the accepted difference
  `abs + rel * |reference|`, optionally for a time window `[t_start, t_end)`,
  with a mandatory reason. A logged property without a row is an error.

`cargo test` uses the committed goldens and needs no C++ build. Regenerating
needs a JSBSim checkout with no tracked changes, CMake, Ninja and a C++
compiler. The checkout is only read; the build goes to `target/oracle/`.

```sh
export JSBSIM_ROOT=~/jsbsim
cargo run -p oracle -- build               # configure and build C++ JSBSim, compile the driver
cargo run -p oracle -- generate ball       # rewrite oracle/goldens/ball/{reference.csv,manifest.toml}
cargo run -p oracle -- check-drift         # regenerate all cases and compare with the goldens
scripts/oracle-drift.sh                    # the same, as run by CI
cargo run -p oracle -- compare --case ball oracle/goldens/ball/reference.csv candidate.csv
```

`compare` and `check-drift` exit with status 1 and report the first divergence:
time, property, both values and the tolerance applied.

To add a case, create `oracle/goldens/<case>/manifest.toml` with a `[case]`
table, add tolerance rows for `Time` and each property, and run `generate`.
