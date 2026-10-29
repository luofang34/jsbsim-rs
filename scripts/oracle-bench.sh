#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Optional, run explicitly: times frames of the Rust port (release build)
# against the C++ driver built from JSBSIM_ROOT (default ~/jsbsim) on the
# cases of oracle/bench.toml, and logs ns per frame, the Rust/C++ ratio and
# frames per second. Fails only if a case cannot run, never on the ratio.
# Arguments name cases; all run when there are none.
#
# It runs twice: with this workspace's release profile, and with Cargo's
# default release profile, which is what a crate depending on jsbsim gets
# unless it sets its own (a dependency's [profile] does not apply).
set -euo pipefail
cd "$(dirname "$0")/.."

export JSBSIM_ROOT="${JSBSIM_ROOT:-$HOME/jsbsim}"
export NO_COLOR=1
echo "== workspace release profile (Cargo.toml [profile.release])"
cargo run --release -q -p oracle -- bench "$@"
echo "== Cargo's default release profile, as a dependent builds it"
CARGO_TARGET_DIR=target/bench-default-profile \
  cargo run --release -q -p oracle \
  --config 'profile.release.codegen-units=16' -- bench "$@"
