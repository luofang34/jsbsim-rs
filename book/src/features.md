# Cargo features

| Feature | For | |
|---|---|---|
| `fs` | you | adds [`FileResolver`](api/jsbsim/struct.FileResolver.html), which reads model files from a directory |
| `__oracle` | `tools/oracle` only | exposes the models' internals to the oracle; not covered by semantic versioning |
| `__platform-libm` | `scripts/oracle-sincos.sh` only | uses the platform's maths library instead of the bundled one, which changes results |

Features whose names begin with `__` are internal hooks for this
repository's own tools. Leave them off: they promise nothing between
releases, and `__platform-libm` gives up determinism across targets.
[`scripts/check-package.sh`](https://github.com/luofang34/jsbsim-rs/blob/main/scripts/check-package.sh)
keeps `fs` the only public feature.

The features are declared in the crate's
[manifest](https://github.com/luofang34/jsbsim-rs/blob/main/crates/jsbsim/Cargo.toml),
whose comments say what each one does.
