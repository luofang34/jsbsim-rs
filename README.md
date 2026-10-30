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

`scripts/ci.sh` runs `cargo fmt --check`, Clippy with `-D warnings`, the tests
and doc tests, `cargo doc` with `-D missing_docs -D broken_intra_doc_links`, a
release build, the `wasm32-unknown-unknown` build of `jsbsim`, and the checks in
`scripts/check-*.sh`. One of them, `scripts/check-public-api.sh`, compares the
public API of `jsbsim` with `crates/jsbsim/public-api.txt` and needs
`cargo install --locked cargo-public-api@0.52.0`; after an intended API change,
`scripts/check-public-api.sh --bless` rewrites the listing.

## Oracle

`tools/oracle` compares this port against C++ JSBSim. Its inputs and outputs
live in `oracle/`:

- `oracle/goldens/<case>/manifest.toml`: what the case runs (`[case]`) and the
  upstream commit of the committed reference (`[provenance]`). A `script` case
  runs a JSBSim script and samples properties into `reference.csv` at full
  double precision. A `transcript` case feeds `input.txt` to a probe mode of
  the driver (for example `properties`, which exercises the property tree) and
  stores its output in `reference.txt`; the Rust port must reproduce it line
  for line.
- `oracle/inputs/`: copies of JSBSim files that cases read, at their upstream
  paths, so `cargo test` needs no checkout; `check-xml` fails if they drift.
- `tools/oracle/cpp/`: the C++ driver, one `probe_*.cpp` per probe mode, with
  Rust twins in `tools/oracle/src/probe/`.
- `oracle/tolerances.toml`: per case and property, the accepted difference
  `abs + rel * |reference|`, optionally for a time window `[t_start, t_end)`,
  with a mandatory reason, and for angles on a branch cut a `period` (2π or
  360) by which differences are wrapped. A logged property without a row is
  an error. A reason stating `bound 10x that` must have a bound of 10 to 20
  times the largest difference `port` observes under the row (ten times,
  rounded up to one significant figure); one stating `bound at least <m>x
  that` (m ≥ 2) must have at least that margin; `port` fails otherwise,
  and on a claiming row that observes no difference. A
  transcript case with rows is compared number by number under the row for
  `<op>/<tag>`, then `<op>/*`, then `*`; without rows it must match exactly.

`cargo test` uses the committed goldens and needs no C++ build. Regenerating
needs a JSBSim checkout with no tracked changes, CMake, Ninja and a C++
compiler. The checkout is only read; the build goes to `target/oracle/` and
uses `-ffp-contract=off`, because Rust never fuses multiply-adds.

```sh
export JSBSIM_ROOT=~/jsbsim
cargo run -p oracle -- build               # configure and build C++ JSBSim, compile the driver
cargo run -p oracle -- generate ball       # rewrite oracle/goldens/ball/{reference.csv,manifest.toml}
cargo run -p oracle -- check-drift         # regenerate all cases and compare with the goldens
cargo run -p oracle -- check-xml           # parse every JSBSim XML file and load its functions in C++ and Rust
cargo run -p oracle -- port function-ops   # run the Rust side of a transcript case and report differences
scripts/oracle-ci.sh                       # check-drift and check-xml, as run by CI
scripts/oracle-bench.sh                    # ns/frame of Rust vs C++ on the oracle/bench.toml cases
scripts/oracle-sincos.sh                   # C++ without sin/cos fusion vs the port on the platform libm, bit for bit
scripts/wasm-determinism.sh                # the port natively vs as wasm32-wasip1 under Node, bit for bit (oracle/determinism.toml)
cargo run -p oracle -- compare --case ball oracle/goldens/ball/reference.csv candidate.csv
```

`compare` and `check-drift` exit with status 1 and report the first divergence:
time, property, both values and the tolerance applied.

To add a case, create `oracle/goldens/<case>/manifest.toml` with a `[case]`
table (and `input.txt` for a transcript), add tolerance rows for `Time` and
each property of a series case, and run `generate`.
