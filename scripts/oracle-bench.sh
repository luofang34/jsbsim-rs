#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Optional, run explicitly: times frames of the Rust port (release build)
# against the C++ driver built from JSBSIM_ROOT (default ~/jsbsim) on the
# cases of oracle/bench.toml, and logs ns per frame, the Rust/C++ ratio and
# frames per second. Arguments name cases; all run when there are none.
#
# It runs twice: with this workspace's release profile, and with Cargo's
# default release profile, which is what a crate depending on jsbsim gets
# unless it sets its own (a dependency's [profile] does not apply).
#
# Fails if a case cannot run, or if c172x's median Rust/C++ ratio is over
# 1.05 under the workspace profile or 1.10 under the default profile (when
# c172x runs); both profiles run either way.
set -euo pipefail
cd "$(dirname "$0")/.."

export JSBSIM_ROOT="${JSBSIM_ROOT:-$HOME/jsbsim}"
export NO_COLOR=1
status=0
echo "== workspace release profile (Cargo.toml [profile.release])"
cargo run --release -q -p oracle -- bench --max-ratio c172x=1.05 "$@" || status=1
echo "== Cargo's default release profile, as a dependent builds it"
CARGO_TARGET_DIR=target/bench-default-profile \
  cargo run --release -q -p oracle \
  --config 'profile.release.codegen-units=16' -- bench --max-ratio c172x=1.10 "$@" || status=1
exit "$status"
