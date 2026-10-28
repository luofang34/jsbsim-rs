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
