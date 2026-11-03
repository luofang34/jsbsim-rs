# Releasing

[`.github/workflows/release.yml`](https://github.com/luofang34/jsbsim-rs/blob/main/.github/workflows/release.yml)
publishes `jsbsim` to crates.io:

1. Set `version` in
   [`crates/jsbsim/Cargo.toml`](https://github.com/luofang34/jsbsim-rs/blob/main/crates/jsbsim/Cargo.toml),
   update `Cargo.lock`, and merge to `main` with CI green.
2. Push a tag `v<version>` matching that version.
3. A reviewer of the `release` environment approves the run.

The workflow checks that the tag is the crate's version, runs
[`scripts/check-package.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/check-package.sh),
and publishes through crates.io trusted publishing, which exchanges the
run's OIDC token for a short-lived registry token, so no registry token is
stored in the repository. Started by hand with an existing tag, it
publishes that tag's source, as a dry run by default, for a tag whose own
run failed before publishing.

The manual and the API documentation on GitHub Pages are published from
`main` by [`book.yml`](https://github.com/luofang34/jsbsim-rs/blob/main/.github/workflows/book.yml)
on every push, so they describe `main`; docs.rs keeps the API documentation
of each released version.
