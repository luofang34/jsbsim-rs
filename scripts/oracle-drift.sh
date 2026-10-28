#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Rebuilds C++ JSBSim from JSBSIM_ROOT (default ~/jsbsim), regenerates every
# golden case under target/ and fails if any departs from the committed
# reference beyond oracle/tolerances.toml, or if the checkout's commit differs
# from the one recorded in a manifest.
set -euo pipefail
cd "$(dirname "$0")/.."

export JSBSIM_ROOT="${JSBSIM_ROOT:-$HOME/jsbsim}"
NO_COLOR=1 cargo run --release -q -p oracle -- check-drift "$@"
