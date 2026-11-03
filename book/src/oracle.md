# The oracle tool

[`tools/oracle`](https://github.com/luofang34/jsbsim-rs/tree/main/tools/oracle)
builds C++ JSBSim with a driver of its own, runs the golden cases in both
C++ and the port, and compares them. `cargo test` uses the committed
goldens and needs none of this; you need it to regenerate goldens, add
cases, or run the gates against C++ locally.

## Setting up

Regenerating needs a JSBSim checkout with no tracked changes at the
goldens' `upstream_commit`, CMake, Ninja and a C++ compiler. The checkout
is only read; the build goes to `target/oracle/` and uses
`-ffp-contract=off`, because Rust never fuses multiply-adds. The goldens
were made on macOS arm64, where CI's oracle job runs, because the C++ side
depends on the platform's maths library.

```sh
export JSBSIM_ROOT=~/jsbsim
cargo run -p oracle -- build               # configure and build C++ JSBSim, compile the driver
```

## Commands

```sh
cargo run -p oracle -- generate ball       # rewrite oracle/goldens/ball/{reference.csv,manifest.toml}
cargo run -p oracle -- check-drift         # regenerate all cases and compare with the goldens
cargo run -p oracle -- check-xml           # parse every JSBSim XML file and load its functions in C++ and Rust
cargo run -p oracle -- check-models        # load every aircraft in C++ and Rust
cargo run -p oracle -- port function-ops   # run the Rust side of a case and report differences
cargo run -p oracle -- compare --case ball oracle/goldens/ball/reference.csv candidate.csv
scripts/oracle-ci.sh                       # every oracle gate, as CI's oracle job runs them
scripts/oracle-bench.sh                    # ns/frame of Rust vs C++ on the oracle/bench.toml cases
scripts/oracle-sincos.sh                   # C++ without sin/cos fusion vs the port on the platform libm, bit for bit
scripts/wasm-determinism.sh                # the port natively vs as wasm32-wasip1 under Node, bit for bit
scripts/oracle-nasa.sh                     # the NASA check cases
```

`cargo run -p oracle -- --help` lists every command, and `--help` after a
command explains its options; the
[command-line definition](https://github.com/luofang34/jsbsim-rs/blob/main/tools/oracle/src/cli.rs)
is the authority. `compare` and `check-drift` exit with status 1 and
report the first divergence: time, property, both values and the tolerance
applied.

## Adding a case

1. Create `oracle/goldens/<case>/manifest.toml` with a `[case]` table: for a
   script case, the script and the properties to sample; for a transcript
   case, the probe mode, with an `input.txt` beside it.
2. If the case reads JSBSim files not yet in `oracle/inputs`, copy them
   there at their upstream paths.
3. For a script case, add rows to `oracle/tolerances.toml` for `Time` and
   each property, each with its reason.
4. Run `generate <case>`, then `port <case>`, and commit the reference with
   the manifest.

[`scripts/verify-goldens-committed.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/verify-goldens-committed.sh)
fails CI when a golden, input or scenario file on disk is not in the
commit. A case added to the goldens joins the sincos gate's list in
[`tools/oracle/src/sincos.rs`](https://github.com/luofang34/jsbsim-rs/blob/main/tools/oracle/src/sincos.rs).
