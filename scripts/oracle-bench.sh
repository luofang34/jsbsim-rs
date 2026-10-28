#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Optional, run explicitly: times frames of the Rust port (release build)
# against the C++ driver built from JSBSIM_ROOT (default ~/jsbsim) on the
# cases of oracle/bench.toml, and logs ns per frame, the Rust/C++ ratio and
# frames per second. Fails only if a case cannot run, never on the ratio.
# Arguments name cases; all run when there are none.
set -euo pipefail
cd "$(dirname "$0")/.."

export JSBSIM_ROOT="${JSBSIM_ROOT:-$HOME/jsbsim}"
export NO_COLOR=1
cargo run --release -q -p oracle -- bench "$@"
