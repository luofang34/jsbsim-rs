#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Opt-in diagnostic against the C++ JSBSim in JSBSIM_ROOT (default
# ~/jsbsim): builds it into target/oracle-sincos with -fno-builtin-sin
# -fno-builtin-cos, so Clang keeps sin and cos as separate calls instead of
# fusing them into one sincos, regenerates the references of the cases in
# tools/oracle/src/sincos.rs under target/oracle-sincos, and requires the
# port, linked with the platform maths library (tools/libm-platform in
# place of the libm crate), to reproduce them bit for bit. Exits non-zero
# at any difference. The gate's build, goldens and tolerances are not used.
# Usage: scripts/oracle-sincos.sh [case...]
set -euo pipefail
cd "$(dirname "$0")/.."

export JSBSIM_ROOT="${JSBSIM_ROOT:-$HOME/jsbsim}"
export NO_COLOR=1
work=target/oracle-sincos
mkdir -p "$work"
# The patch rewrites Cargo.lock's libm entry; put it back afterwards.
command cp -f Cargo.lock "$work/Cargo.lock.saved"
trap 'command cp -f "$work/Cargo.lock.saved" Cargo.lock' EXIT
CARGO_TARGET_DIR="$work/cargo" cargo run --release -q -p oracle \
  --config "patch.crates-io.libm.path=\"$PWD/tools/libm-platform\"" \
  -- check-sincos "$@"
