# Building, testing and CI

The toolchain is pinned in
[`rust-toolchain.toml`](https://github.com/luofang34/jsbsim-rs/blob/main/rust-toolchain.toml).

```sh
cargo build
cargo test --workspace --all-targets
scripts/ci.sh            # every CI gate, in order
scripts/ci.sh clippy     # one gate; scripts/ci.sh --list names them
```

## The gates

[`scripts/ci.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/ci.sh)
runs, in order and stopping at the first failure:

1. `cargo fmt --check`;
2. Clippy with `-D warnings`, on the workspace and on `jsbsim` alone as a
   dependent builds it, where every public function returning a `Result`
   must document its errors;
3. the tests, including every example in
   [`crates/jsbsim/examples`](https://github.com/luofang34/jsbsim-rs/tree/main/crates/jsbsim/examples),
   which runs as a test;
4. the doc tests, including the crate README's examples;
5. `cargo doc` with missing docs and broken intra-doc links denied, on the
   workspace and as docs.rs builds the crate;
6. a release build, and the `wasm32-unknown-unknown` build of `jsbsim`;
7. the structural checks, `scripts/check-*.sh`, and the scripts' own tests,
   `scripts/*.test.sh`.

The structural checks enforce what rustc and Clippy do not:

| Check | Fails when |
|---|---|
| [`check-structure.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/check-structure.sh) | a file or function is too long, a module uses `mod.rs` or a catch-all name, a file lacks its SPDX line or its upstream copyright, a counter does not wrap, or code prints instead of logging |
| [`check-public-api.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/check-public-api.sh) | the public API differs from [`crates/jsbsim/public-api.txt`](https://github.com/luofang34/jsbsim-rs/blob/main/crates/jsbsim/public-api.txt); after an intended change, `--bless` rewrites the listing |
| [`check-package.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/check-package.sh) | the published package would ship more than sources, README and licences, pin a dependency, or add a public feature |
| [`check-ci-workflow.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/check-ci-workflow.sh) | the CI workflow's steps and these gates differ, or an action is not pinned to a commit |
| [`check-fdm-borrows.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/check-fdm-borrows.sh) | model code copies its load-time lists, or moves a model out of the executive, to get past the borrow checker |
| [`check-book.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/check-book.sh) | this manual could drift from the code (below) |

`check-public-api.sh` needs `cargo install --locked cargo-public-api` at
the version the script names, and `check-book.sh` needs `mdbook` at the
version it names.

## GitHub Actions

[`.github/workflows/ci.yml`](https://github.com/luofang34/jsbsim-rs/blob/main/.github/workflows/ci.yml)
runs the same gates on every push and pull request: `quality-gates` runs
each `scripts/ci.sh` gate as a step on Linux, `wasm-determinism` compares
native and WebAssembly results, and `oracle` runs
[`scripts/oracle-ci.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/oracle-ci.sh)
on macOS arm64 against C++ JSBSim at the goldens' upstream commit (see
[Verification](verification.md)).
[`oracle-extended.yml`](https://github.com/luofang34/jsbsim-rs/blob/main/.github/workflows/oracle-extended.yml)
runs the NASA check cases weekly and the benchmark on request;
[`upstream-sync.yml`](https://github.com/luofang34/jsbsim-rs/blob/main/.github/workflows/upstream-sync.yml)
tracks upstream JSBSim; [`book.yml`](https://github.com/luofang34/jsbsim-rs/blob/main/.github/workflows/book.yml)
publishes this manual.

## This manual

The manual lives in
[`book/`](https://github.com/luofang34/jsbsim-rs/tree/main/book) and is
built with [mdBook](https://rust-lang.github.io/mdBook/), together with the
API documentation of the same commit under `api/`:

```sh
scripts/check-book.sh                  # build into target/book and check it
mdbook serve book                      # live preview, without the API pages
```

`check-book.sh` keeps the manual tied to the code:

- Rust listings are never pasted. Each is an `{{#include}}` of an anchored
  region (`// ANCHOR: name` to `// ANCHOR_END: name`) of an example in
  `crates/jsbsim/examples`, and every example runs as a test, so a listing
  that stops compiling, or stops doing what the manual says, fails CI.
- An include of a missing file or anchor fails, as does an include by line
  numbers.
- A link to a file in this repository must name a path that exists, and
  never a line number.
- A link to the API documentation (`api/jsbsim/...`) must name a page and
  anchor that rustdoc generates, so renaming or removing an item breaks the
  build of the manual until the manual is updated.
- Every chapter is in `SUMMARY.md`, and the README's quickstart is part of
  the crate README, whose examples run as doc tests.

To add a listing, put the code in an example (or add an example, with
`test = true` in the crate manifest and a `#[test]` that calls `main`),
mark it with anchors, and include it.

## Code conventions

The lint policy is in the workspace
[`Cargo.toml`](https://github.com/luofang34/jsbsim-rs/blob/main/Cargo.toml)
and [`clippy.toml`](https://github.com/luofang34/jsbsim-rs/blob/main/clippy.toml):
no `unsafe`, no `unwrap`, `expect` or `panic!` outside tests, diagnostics
through `tracing`, never `println!`. While the port reproduces C++ JSBSim
bit for bit, lints whose suggestions round differently from the C++ (fused
multiply-adds, `hypot`, `ln_1p`) or obscure the line-for-line match are off,
in blocks marked "C++ mirroring" in both files and in
`tools/libm-platform/Cargo.toml`; deleting those blocks is the switch to
idiomatic Rust. Never apply Clippy's floating-point suggestions while
parity matters.

Each source file ported from JSBSim keeps the copyright lines of the C++
files it translates and names them, so the C++ counterpart of any Rust
module is one line away.
