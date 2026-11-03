# Verification against C++ JSBSim

The port is checked against C++ JSBSim itself, built from a pinned upstream
commit, which acts as the *oracle*: the reference whose output the port
must reproduce. Every check below runs in CI.

## Golden cases

Each case under
[`oracle/goldens`](https://github.com/luofang34/jsbsim-rs/tree/main/oracle/goldens)
has a `manifest.toml` saying what it runs (`[case]`) and which upstream
commit, driver and compiler flags made its committed reference
(`[provenance]`). There are two kinds:

- A **script** case runs a JSBSim script (from JSBSim's own `scripts/`, or
  a variant in
  [`oracle/derived`](https://github.com/luofang34/jsbsim-rs/tree/main/oracle/derived)
  or [`oracle/scenarios`](https://github.com/luofang34/jsbsim-rs/tree/main/oracle/scenarios))
  and samples properties into `reference.csv` at full double precision. The
  port runs the same script and must match each sample.
- A **transcript** case feeds `input.txt` to a *probe* mode of the C++
  driver, which exercises one part of JSBSim directly (the property tree,
  functions, tables, flight-control components, trim, geodesy and so on),
  and stores its output in `reference.txt`. The port's twin of the probe
  must reproduce it line for line.

The cases span JSBSim's aircraft, from the ball and the Cessna 172 to the
737, F-16, X-15, helicopters, airships and multirotors, and every engine
and thruster type. The files JSBSim cases read are copied into
[`oracle/inputs`](https://github.com/luofang34/jsbsim-rs/tree/main/oracle/inputs)
at their upstream paths, so `cargo test` needs no JSBSim checkout, and the
oracle fails if they drift from upstream.

## Tolerances

[`oracle/tolerances.toml`](https://github.com/luofang34/jsbsim-rs/blob/main/oracle/tolerances.toml)
gives, per case and property, the accepted difference
`abs + rel * |reference|`, optionally for a time window `[t_start, t_end)`,
and for angles on a branch cut a `period` (2π or 360) by which differences
wrap. Every row states its reason, and a logged property without a row is
an error. Many rows demand bit-identical results.

Differences come almost entirely from the last bits of transcendental
functions: the port's `libm` and Apple's platform library can differ by an
ulp, and Clang fuses `sin`/`cos` pairs. A tolerance row whose reason
claims a margin (`bound 10x that`, `bound at least <m>x that`) is checked:
the port must observe a difference whose margin under the bound is what the
reason says, so rows cannot quietly grow loose. A transcript case with rows
is compared number by number under the row for `<op>/<tag>`, then `<op>/*`,
then `*`; without rows it must match exactly.

## Bit for bit, where the maths library allows

A tolerance row wide enough for libm differences could hide a real
regression. So the sincos gate rebuilds C++ JSBSim with `sin` and `cos` as
separate calls and links the port against the same platform maths library,
and then requires **every** case to match bit for bit, with no exceptions
([`scripts/oracle-sincos.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/oracle-sincos.sh)).

## Every aircraft loads the same

The model check loads every aircraft in JSBSim's tree in C++ and in the
port, and compares the properties each binds; the expected outcome is
committed in
[`oracle/model-loading.txt`](https://github.com/luofang34/jsbsim-rs/blob/main/oracle/model-loading.txt).
The XML check parses every XML file in JSBSim's tree with both, and loads
every function in it.

## Native and WebAssembly agree

[`scripts/wasm-determinism.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/wasm-determinism.sh)
runs the unit tests as `wasm32-wasip1` under Node, records the cases of
[`oracle/determinism.toml`](https://github.com/luofang34/jsbsim-rs/blob/main/oracle/determinism.toml)
natively and as WebAssembly, restores native snapshots in WebAssembly, and
fails on the first bit that differs.

## Snapshots

Snapshot tests in
[`tools/oracle/tests`](https://github.com/luofang34/jsbsim-rs/tree/main/tools/oracle/tests)
run each case straight through and again with a snapshot taken and restored
part way, and require the two traces to agree to the bit; they also check
that restoring is atomic under corruption, that taking and restoring
allocate nothing after the first, and that the format matches a committed
golden byte stream.

## NASA check cases

The weekly extended workflow runs the check cases of NASA's flight
simulation verification report (TM-2015-218675) through
[`scripts/oracle-nasa.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/oracle-nasa.sh):
C++ JSBSim and the port each fly them, and both are compared with the
spread of NASA's simulators. A sample passes inside that spread; where C++
JSBSim misses, the port must miss on exactly the same samples.
The cases and their expected misses are in
[`oracle/nasa/manifest.toml`](https://github.com/luofang34/jsbsim-rs/blob/main/oracle/nasa/manifest.toml).

The F-16 cases fly a hand translation of NASA's DAVE-ML F-16. The port
matches C++ JSBSim on this hand-translated DAVE-ML F-16; the model lands
near, but mostly outside, NASA's sims-4/5 envelope; causes are not isolated
and translation error is not excluded.

## Performance

[`scripts/oracle-bench.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/oracle-bench.sh)
times frames of the port against C++ JSBSim on the cases of
[`oracle/bench.toml`](https://github.com/luofang34/jsbsim-rs/blob/main/oracle/bench.toml).
It runs on request, since shared CI runners make the timings noisy.

## Following upstream

A weekly workflow lists the upstream JSBSim commits since the goldens'
pinned commit that touch C++ sources, model data or tests, and keeps an
open `upstream-sync` issue with them
([`scripts/upstream-report.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/upstream-report.sh)).
