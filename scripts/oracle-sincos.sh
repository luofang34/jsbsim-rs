#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Run by oracle-ci.sh, or alone, against the C++ JSBSim in JSBSIM_ROOT (default
# ~/jsbsim): builds it into target/oracle-sincos with -fno-builtin-sin
# -fno-builtin-cos, so Clang keeps sin and cos as separate calls instead of
# fusing them into one sincos, regenerates the references of the cases in
# tools/oracle/src/sincos.rs under target/oracle-sincos, and requires the
# port, linked with the platform maths library (tools/libm-platform in
# place of the libm crate, and jsbsim::elementary forwarding to it through
# the platform-libm feature), to reproduce them bit for bit. Exits non-zero
# at any difference. The gate's build, goldens and tolerances are not used.
# Usage: scripts/oracle-sincos.sh [case...]
set -euo pipefail
cd "$(dirname "$0")/.."

export JSBSIM_ROOT="${JSBSIM_ROOT:-$HOME/jsbsim}"
export NO_COLOR=1
work=target/oracle-sincos
# The libm patch rewrites the lock file of the workspace it runs in, so it
# runs in a throwaway workspace: a copy of the manifest and lock, with the
# sources linked in. The real Cargo.lock is never written; the check at the
# end proves it.
ws="$work/workspace"
rm -rf "$ws"
mkdir -p "$ws"
command cp -f Cargo.toml Cargo.lock "$ws/"
for entry in crates tools clippy.toml rust-toolchain.toml; do
  ln -s "$PWD/$entry" "$ws/$entry"
done
lock_before=$(shasum -a 256 Cargo.lock)
status=0
CARGO_TARGET_DIR="$work/cargo" cargo run --release -q \
  --manifest-path "$ws/Cargo.toml" -p oracle \
  --config "patch.crates-io.libm.path=\"$PWD/tools/libm-platform\"" \
  --features jsbsim/platform-libm -- check-sincos "$@" || status=$?
if [ "$(shasum -a 256 Cargo.lock)" != "$lock_before" ]; then
  echo "oracle-sincos: Cargo.lock changed during the run" >&2
  status=1
fi
exit "$status"
