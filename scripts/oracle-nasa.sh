#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Optional, run explicitly: downloads the NASA/TM-2015-218675 check-case data
# pinned in oracle/nasa/manifest.toml into target/oracle/nasa, fails if the
# site is unreachable or an archive no longer matches its sha256, then runs
# the check-cases in C++ JSBSim (JSBSIM_ROOT, default ~/jsbsim) and in the
# Rust port and compares both with the spread of the NASA simulators.
# Arguments name check-cases (e.g. Atmos_02); all run when there are none.
# Then runs the F-16 checks on both: mass properties against
# F16_inertia.dml, the static shots of F16_aero.dml and F16_prop.dml, and
# the control law frame by frame against F16_control.dml and F16_gnc.dml.
# NASA_OFFLINE=1 skips the download and uses the archives already in
# target/oracle/nasa, which are still checked against their pins.
set -euo pipefail
cd "$(dirname "$0")/.."

export JSBSIM_ROOT="${JSBSIM_ROOT:-$HOME/jsbsim}"
export NO_COLOR=1
if [ "${NASA_OFFLINE:-0}" != 1 ]; then
  cargo run --release -q -p oracle -- nasa fetch
fi
cargo run --release -q -p oracle -- nasa compare "$@"
for check in mass shots control-law; do
  cargo run --release -q -p oracle -- nasa "$check"
done
