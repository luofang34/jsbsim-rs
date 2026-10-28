#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Optional, run explicitly: downloads the NASA/TM-2015-218675 check-case data
# pinned in oracle/nasa/manifest.toml into target/oracle/nasa, fails if the
# site is unreachable or an archive no longer matches its sha256, then runs
# the check-cases in C++ JSBSim (JSBSIM_ROOT, default ~/jsbsim) and in the
# Rust port and compares both with the spread of the NASA simulators.
# Arguments name check-cases (e.g. Atmos_02); all run when there are none.
set -euo pipefail
cd "$(dirname "$0")/.."

export JSBSIM_ROOT="${JSBSIM_ROOT:-$HOME/jsbsim}"
export NO_COLOR=1
cargo run --release -q -p oracle -- nasa fetch
cargo run --release -q -p oracle -- nasa compare "$@"
